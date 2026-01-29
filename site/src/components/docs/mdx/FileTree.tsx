import { For, Show, createSignal, ParentProps } from "solid-js";
import { ChevronRight, ChevronDown, Folder, File } from "lucide-solid";

interface FileTreeItem {
  name: string;
  type: "file" | "folder";
  children?: FileTreeItem[];
  highlight?: boolean;
}

interface FileTreeProps {
  data: FileTreeItem[];
}

export function FileTree(props: FileTreeProps) {
  return (
    <div class="my-6 rounded-lg border border-slate-700 bg-slate-800/50 p-4 font-mono text-sm">
      <For each={props.data}>
        {(item) => <FileTreeNode item={item} depth={0} />}
      </For>
    </div>
  );
}

interface FileTreeNodeProps {
  item: FileTreeItem;
  depth: number;
}

function FileTreeNode(props: FileTreeNodeProps) {
  const [expanded, setExpanded] = createSignal(true);
  const isFolder = () => props.item.type === "folder";

  return (
    <div>
      <div
        class="flex cursor-pointer items-center gap-1.5 rounded px-2 py-1 transition-colors hover:bg-white/5"
        classList={{
          "text-yellow-400": props.item.highlight,
          "text-slate-300": !props.item.highlight,
        }}
        style={{ "padding-left": `${props.depth * 16 + 8}px` }}
        onClick={() => isFolder() && setExpanded((prev) => !prev)}
      >
        <Show when={isFolder()}>
          <Show
            when={expanded()}
            fallback={<ChevronRight size={14} class="text-slate-500" />}
          >
            <ChevronDown size={14} class="text-slate-500" />
          </Show>
        </Show>
        <Show when={!isFolder()}>
          <span class="w-3.5" />
        </Show>

        <Show when={isFolder()} fallback={<File size={14} class="text-slate-500" />}>
          <Folder size={14} class="text-indigo-400" />
        </Show>

        <span>{props.item.name}</span>
      </div>

      <Show when={isFolder() && expanded() && props.item.children}>
        <For each={props.item.children}>
          {(child) => <FileTreeNode item={child} depth={props.depth + 1} />}
        </For>
      </Show>
    </div>
  );
}

export default FileTree;
