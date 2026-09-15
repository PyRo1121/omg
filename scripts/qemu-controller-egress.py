#!/usr/bin/env python3
"""Scope a Docker controller to public HTTP(S) and explicit DNS resolvers."""
import argparse
import hashlib
import ipaddress
import json
import re
import subprocess

PRIVATE = ("0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8",
           "169.254.0.0/16", "172.16.0.0/12", "192.0.0.0/24", "192.168.0.0/16",
           "198.18.0.0/15", "224.0.0.0/4", "240.0.0.0/4")
RESOLVERS = ("1.1.1.1", "9.9.9.9")


def execute(argv, check=True):
    return subprocess.run(argv, capture_output=True, text=True, timeout=15, check=check)


def chain_name(controller):
    if not re.fullmatch(r"omg-qemu-run-[a-zA-Z0-9]{6,16}", controller):
        raise ValueError("not an owned QEMU controller name")
    return "OMGQ" + hashlib.sha256(controller.encode()).hexdigest()[:16]


def rules(address):
    ip = str(ipaddress.IPv4Address(address))
    yield ["!", "-s", ip, "-j", "RETURN"]
    # INPUT traffic to the runner itself is denied as well as routed private
    # destinations. DOCKER-USER alone would not cover the bridge host address.
    yield ["-m", "addrtype", "--dst-type", "LOCAL", "-j", "REJECT"]
    for destination in PRIVATE:
        yield ["-d", destination, "-j", "REJECT"]
    for resolver in RESOLVERS:
        for protocol in ("udp", "tcp"):
            yield ["-d", resolver, "-p", protocol, "--dport", "53", "-j", "RETURN"]
    yield ["-p", "tcp", "-m", "multiport", "--dports", "80,443", "-j", "RETURN"]
    yield ["-j", "REJECT"]


def install(controller):
    chain = chain_name(controller)
    container, = json.loads(execute(["docker", "inspect", controller]).stdout)
    host = container["HostConfig"]
    networks = container["NetworkSettings"]["Networks"]
    dropped = {cap.removeprefix("CAP_") for cap in host.get("CapDrop") or []}
    if (container["Name"] != "/" + controller or container["State"]["Running"] is not True
            or list(networks) != ["bridge"] or networks["bridge"].get("GlobalIPv6Address")
            or host.get("Privileged") is not False
            or not {"NET_RAW", "NET_ADMIN"}.issubset(dropped)
            or host.get("Dns") != list(RESOLVERS)):
        observed = dict(name=container["Name"], running=container["State"]["Running"],
                        networks=list(networks), privileged=host.get("Privileged"),
                        cap_drop=host.get("CapDrop"), dns=host.get("Dns"),
                        ipv6=networks.get("bridge", {}).get("GlobalIPv6Address"))
        raise ValueError("controller network/capability configuration is not confined: " + json.dumps(observed))
    address = str(ipaddress.IPv4Address(networks["bridge"]["IPAddress"]))
    execute(["iptables", "-w", "5", "-S", "DOCKER-USER"])
    execute(["iptables", "-w", "5", "-N", chain])
    for rule in rules(address):
        execute(["iptables", "-w", "5", "-A", chain] + rule)
    for parent in ("DOCKER-USER", "INPUT"):
        execute(["iptables", "-w", "5", "-I", parent, "1", "-j", chain])
    # A failed connect alone could be missing routing. Require the installed
    # metadata-deny rule's packet counter to increase as positive evidence.
    probe = execute(["docker", "exec", controller, "bash", "-c",
                     "timeout 3 bash -c 'exec 3<>/dev/tcp/169.254.169.254/80'"], check=False)
    counters = execute(["iptables", "-w", "5", "-L", chain, "-n", "-v", "-x"]).stdout
    matched = [line.split() for line in counters.splitlines()
               if "169.254.0.0/16" in line and "REJECT" in line]
    if probe.returncode == 0 or len(matched) != 1 or int(matched[0][0]) < 1:
        raise ValueError("metadata rejection was not observed at the firewall")
    return dict(schema_version=1, chain=chain, metadata_block_verified=True,
                scope="public-http-https-and-explicit-dns", ipv6="unconfigured")


def remove(controller):
    chain = chain_name(controller)
    names = execute(["docker", "ps", "-a", "--format", "{{.Names}}"]).stdout.splitlines()
    if controller in names:
        raise ValueError("refusing to remove confinement from an existing controller")
    existing = execute(["iptables", "-w", "5", "-S"]).stdout
    if "-N " + chain not in existing.splitlines():
        return
    for parent in ("DOCKER-USER", "INPUT"):
        if f"-A {parent} -j {chain}" in existing.splitlines():
            execute(["iptables", "-w", "5", "-D", parent, "-j", chain])
    execute(["iptables", "-w", "5", "-F", chain])
    execute(["iptables", "-w", "5", "-X", chain])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("install", "remove"))
    parser.add_argument("controller")
    args = parser.parse_args()
    if args.operation == "install":
        print(json.dumps(install(args.controller)))
    else:
        remove(args.controller)


if __name__ == "__main__":
    main()
