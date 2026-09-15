import importlib.util
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("egress", Path(__file__).with_name("qemu-controller-egress.py"))
EGRESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EGRESS)
NAME = "omg-qemu-run-ABC123"


class EgressTests(unittest.TestCase):
    def test_rules_reject_private_and_runner_destinations_before_allowing_web(self):
        rules = list(EGRESS.rules("172.17.0.2"))
        web = rules.index(["-p", "tcp", "-m", "multiport", "--dports", "80,443", "-j", "RETURN"])
        self.assertEqual(rules[0], ["!", "-s", "172.17.0.2", "-j", "RETURN"])
        self.assertLess(rules.index(["-m", "addrtype", "--dst-type", "LOCAL", "-j", "REJECT"]), web)
        for network in EGRESS.PRIVATE:
            self.assertLess(rules.index(["-d", network, "-j", "REJECT"]), web)
        self.assertIn(["-d", "168.63.129.16/32", "-j", "REJECT"], rules)
        self.assertEqual(rules[-1], ["-j", "REJECT"])

    def test_ntp_is_limited_to_boot_configured_public_servers(self):
        rules = list(EGRESS.rules("172.17.0.2"))
        ntp = [rule for rule in rules if "123" in rule]
        self.assertEqual(ntp, [["-d", server, "-p", "udp", "--dport", "123", "-j", "RETURN"]
                               for server in EGRESS.TIME_SERVERS])
        source = Path(__file__).with_name("benchmark-qemu.sh").read_text()
        self.assertIn("NTP=" + " ".join(EGRESS.TIME_SERVERS), source)
        self.assertIn("restart --no-block systemd-timesyncd.service", source)

    def test_cleanup_refuses_live_controller_before_touching_firewall(self):
        with patch.object(EGRESS, "execute", return_value=subprocess.CompletedProcess([], 0, NAME + "\n", "")) as run:
            with self.assertRaises(ValueError):
                EGRESS.remove(NAME)
        self.assertEqual(run.call_count, 1)

    def test_invalid_controller_names_are_rejected(self):
        for name in ("other-container", "omg-qemu-run-../host", "--privileged"):
            with self.assertRaises(ValueError):
                EGRESS.chain_name(name)

    def test_network_policy_requires_positive_metadata_counter(self):
        container = dict(Name="/" + NAME, State=dict(Running=True),
                         HostConfig=dict(Privileged=False, CapDrop=["CAP_NET_RAW", "CAP_NET_ADMIN"], Dns=list(EGRESS.RESOLVERS)),
                         NetworkSettings=dict(Networks=dict(bridge=dict(IPAddress="172.17.0.2", GlobalIPv6Address=""))))
        for packets in (0, 1):
            def fake(argv, check=True):
                output = ""
                if argv[:2] == ["docker", "inspect"]:
                    output = json.dumps([container])
                elif "-L" in argv:
                    output = f"{packets} 60 REJECT all -- * * 0.0.0.0/0 169.254.0.0/16\n"
                return subprocess.CompletedProcess(argv, 1 if argv[:2] == ["docker", "exec"] else 0, output, "")
            with patch.object(EGRESS, "execute", side_effect=fake):
                if packets:
                    self.assertTrue(EGRESS.install(NAME)["metadata_block_verified"])
                else:
                    with self.assertRaises(ValueError):
                        EGRESS.install(NAME)


if __name__ == "__main__":
    unittest.main()
