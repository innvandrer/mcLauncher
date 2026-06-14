import { useMemo, useState } from "react";
import { Boxes, Plus, Search } from "lucide-react";
import { InstanceCard } from "@/components/InstanceCard";
import { CreateInstanceModal } from "@/components/CreateInstanceModal";
import { Button, EmptyState, Input } from "@/components/ui";
import { useStore } from "@/store/useStore";

export function InstancesPage() {
  const instances = useStore((s) => s.instances);
  const openInstance = useStore((s) => s.openInstance);
  const [query, setQuery] = useState("");
  const [createOpen, setCreateOpen] = useState(false);

  const filtered = useMemo(
    () =>
      instances.filter((i) =>
        `${i.name} ${i.mcVersion} ${i.loader}`.toLowerCase().includes(query.toLowerCase()),
      ),
    [instances, query],
  );

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between gap-4 px-8 pb-4 pt-6">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Instances</h1>
          <p className="text-sm text-muted-foreground">
            {instances.length} {instances.length === 1 ? "instance" : "instances"}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search…"
              className="w-56 pl-9"
            />
          </div>
          <Button variant="primary" onClick={() => setCreateOpen(true)}>
            <Plus className="h-4 w-4" />
            New instance
          </Button>
        </div>
      </header>

      <div className="scroll-area flex-1 px-8 pb-8">
        {instances.length === 0 ? (
          <EmptyState
            icon={<Boxes className="h-7 w-7" />}
            title="No instances yet"
            description="Create your first instance to pick a Minecraft version, add a mod loader, and start playing."
            action={
              <Button variant="primary" onClick={() => setCreateOpen(true)}>
                <Plus className="h-4 w-4" />
                Create instance
              </Button>
            }
          />
        ) : filtered.length === 0 ? (
          <p className="py-16 text-center text-sm text-muted-foreground">
            No instances match “{query}”.
          </p>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(210px,1fr))] gap-4">
            {filtered.map((inst) => (
              <InstanceCard key={inst.id} instance={inst} onOpen={() => openInstance(inst.id)} />
            ))}
          </div>
        )}
      </div>

      <CreateInstanceModal open={createOpen} onClose={() => setCreateOpen(false)} />
    </div>
  );
}
