<script lang="ts">
    import { onMount } from "svelte";
    import { getBlocks, getBlock, type BlockInfo } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";

    let blocks = $state<BlockInfo[]>([]);
    let selectedBlock = $state<BlockInfo | null>(null);
    let loading = $state(true);

    onMount(async () => {
        try {
            blocks = await getBlocks();
            if (blocks.length > 0) {
                selectedBlock = blocks[0];
            }
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    });

    async function handleSelectBlock(height: number) {
        try {
            selectedBlock = await getBlock(height);
        } catch (e) {
            console.error(e);
        }
    }
</script>

<svelte:head>
    <title>Blocks | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Block Explorer</h1>
        <p class="text-muted-foreground">Browse all blocks on the chain</p>
    </div>

    <div class="grid gap-6 lg:grid-cols-5">
        <!-- Block List -->
        <Card.Root class="lg:col-span-2">
            <Card.Header>
                <Card.Title>Blocks</Card.Title>
                <Card.Description
                    >{blocks.length} blocks on chain</Card.Description
                >
            </Card.Header>
            <Card.Content>
                {#if loading}
                    <div class="flex items-center justify-center h-32">
                        <div
                            class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
                        ></div>
                    </div>
                {:else}
                    <div class="space-y-2 max-h-[500px] overflow-y-auto pr-2">
                        {#each blocks as block}
                            <button
                                type="button"
                                class="w-full text-left p-3 rounded-lg border transition-colors hover:bg-accent cursor-pointer
                  {selectedBlock?.index === block.index
                                    ? 'bg-accent border-primary'
                                    : 'border-transparent'}"
                                onclick={() => handleSelectBlock(block.index)}
                            >
                                <div class="flex items-center justify-between">
                                    <div class="flex items-center gap-3">
                                        <Badge variant="outline"
                                            >#{block.index}</Badge
                                        >
                                        <span
                                            class="font-mono text-xs text-muted-foreground"
                                            >{block.hash.slice(0, 16)}...</span
                                        >
                                    </div>
                                    <Badge variant="secondary"
                                        >{block.transactions} tx</Badge
                                    >
                                </div>
                            </button>
                        {/each}
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>

        <!-- Block Details -->
        <Card.Root class="lg:col-span-3">
            <Card.Header>
                <Card.Title>Block Details</Card.Title>
                <Card.Description
                    >Detailed information about the selected block</Card.Description
                >
            </Card.Header>
            <Card.Content>
                {#if selectedBlock}
                    <div class="space-y-6">
                        <div class="flex items-center gap-4">
                            <div
                                class="flex h-14 w-14 items-center justify-center rounded-lg bg-primary text-primary-foreground"
                            >
                                <span class="text-lg font-bold"
                                    >#{selectedBlock.index}</span
                                >
                            </div>
                            <div>
                                <p class="font-medium">
                                    Block #{selectedBlock.index}
                                </p>
                                <p class="text-sm text-muted-foreground">
                                    {new Date(
                                        selectedBlock.timestamp,
                                    ).toLocaleString()}
                                </p>
                            </div>
                        </div>

                        <Separator />

                        <div class="grid gap-4">
                            <div class="space-y-2">
                                <p
                                    class="text-sm font-medium text-muted-foreground"
                                >
                                    Hash
                                </p>
                                <code
                                    class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >{selectedBlock.hash}</code
                                >
                            </div>
                            <div class="space-y-2">
                                <p
                                    class="text-sm font-medium text-muted-foreground"
                                >
                                    Previous Hash
                                </p>
                                <code
                                    class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >{selectedBlock.previous_hash}</code
                                >
                            </div>
                            <div class="space-y-2">
                                <p
                                    class="text-sm font-medium text-muted-foreground"
                                >
                                    Merkle Root
                                </p>
                                <code
                                    class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >{selectedBlock.merkle_root}</code
                                >
                            </div>
                        </div>

                        <Separator />

                        <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Difficulty
                                </p>
                                <p class="text-lg font-bold">
                                    {selectedBlock.difficulty}
                                </p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Nonce
                                </p>
                                <p class="text-lg font-bold font-mono">
                                    {selectedBlock.nonce}
                                </p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Transactions
                                </p>
                                <p class="text-lg font-bold">
                                    {selectedBlock.transactions}
                                </p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Timestamp
                                </p>
                                <p class="text-sm font-mono">
                                    {new Date(
                                        selectedBlock.timestamp,
                                    ).toLocaleTimeString()}
                                </p>
                            </div>
                        </div>
                    </div>
                {:else}
                    <div
                        class="flex flex-col items-center justify-center h-64 text-muted-foreground"
                    >
                        <span class="text-4xl mb-4">🧱</span>
                        <p>Select a block to view details</p>
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>
    </div>
</div>
