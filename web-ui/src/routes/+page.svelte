<script lang="ts">
    import { onMount } from "svelte";
    import {
        getChainInfo,
        getBlocks,
        type ChainInfo,
        type BlockInfo,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Separator } from "$lib/components/ui/separator";

    let chainInfo = $state<ChainInfo | null>(null);
    let recentBlocks = $state<BlockInfo[]>([]);
    let loading = $state(true);
    let error = $state("");

    onMount(async () => {
        try {
            [chainInfo, recentBlocks] = await Promise.all([
                getChainInfo(),
                getBlocks(),
            ]);
        } catch (e) {
            error = "Failed to connect to blockchain API.";
        } finally {
            loading = false;
        }
    });
</script>

<svelte:head>
    <title>Dashboard | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <!-- Page Header -->
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p class="text-muted-foreground">Overview of your blockchain network</p>
    </div>

    {#if loading}
        <div class="flex items-center justify-center h-64">
            <div class="flex flex-col items-center gap-2">
                <div
                    class="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent"
                ></div>
                <p class="text-sm text-muted-foreground">
                    Loading blockchain data...
                </p>
            </div>
        </div>
    {:else if error}
        <Card.Root class="border-destructive/50 bg-destructive/10">
            <Card.Header>
                <Card.Title class="text-destructive"
                    >Connection Error</Card.Title
                >
            </Card.Header>
            <Card.Content>
                <p class="text-sm text-muted-foreground mb-4">{error}</p>
                <code class="block bg-muted p-3 rounded-lg text-sm">
                    ./target/release/blockchain api start --port 3000
                </code>
            </Card.Content>
        </Card.Root>
    {:else if chainInfo}
        <!-- Stats Grid -->
        <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium"
                        >Chain Height</Card.Title
                    >
                    <span class="text-muted-foreground">🧱</span>
                </Card.Header>
                <Card.Content>
                    <div class="text-2xl font-bold">{chainInfo.height}</div>
                    <p class="text-xs text-muted-foreground">blocks mined</p>
                </Card.Content>
            </Card.Root>

            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium"
                        >Difficulty</Card.Title
                    >
                    <span class="text-muted-foreground">⚡</span>
                </Card.Header>
                <Card.Content>
                    <div class="text-2xl font-bold">{chainInfo.difficulty}</div>
                    <p class="text-xs text-muted-foreground">
                        leading zero bits
                    </p>
                </Card.Content>
            </Card.Root>

            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium"
                        >Total Coins</Card.Title
                    >
                    <span class="text-muted-foreground">💰</span>
                </Card.Header>
                <Card.Content>
                    <div class="text-2xl font-bold">
                        {chainInfo.total_coins.toLocaleString()}
                    </div>
                    <p class="text-xs text-muted-foreground">in circulation</p>
                </Card.Content>
            </Card.Root>

            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium"
                        >Transactions</Card.Title
                    >
                    <span class="text-muted-foreground">📝</span>
                </Card.Header>
                <Card.Content>
                    <div class="text-2xl font-bold">
                        {chainInfo.total_transactions}
                    </div>
                    <p class="text-xs text-muted-foreground">
                        across all blocks
                    </p>
                </Card.Content>
            </Card.Root>
        </div>

        <!-- Latest Hash -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Latest Block</Card.Title>
                <Card.Description
                    >Most recently mined block hash</Card.Description
                >
            </Card.Header>
            <Card.Content>
                <code
                    class="block text-sm text-muted-foreground font-mono break-all bg-muted p-4 rounded-lg"
                >
                    {chainInfo.latest_hash}
                </code>
            </Card.Content>
        </Card.Root>

        <!-- Recent Blocks -->
        <Card.Root>
            <Card.Header>
                <div class="flex items-center justify-between">
                    <div>
                        <Card.Title>Recent Blocks</Card.Title>
                        <Card.Description
                            >Latest blocks on the chain</Card.Description
                        >
                    </div>
                    <Button variant="outline" size="sm" href="/blocks"
                        >View all</Button
                    >
                </div>
            </Card.Header>
            <Card.Content>
                <div class="space-y-4">
                    {#each recentBlocks.slice(0, 5) as block, i}
                        {#if i > 0}
                            <Separator />
                        {/if}
                        <div class="flex items-center justify-between">
                            <div class="flex items-center gap-4">
                                <div
                                    class="flex h-10 w-10 items-center justify-center rounded-lg bg-muted"
                                >
                                    <span class="text-sm font-medium"
                                        >#{block.index}</span
                                    >
                                </div>
                                <div class="space-y-1">
                                    <p
                                        class="text-sm font-medium leading-none font-mono"
                                    >
                                        {block.hash.slice(0, 24)}...
                                    </p>
                                    <p class="text-sm text-muted-foreground">
                                        {new Date(
                                            block.timestamp,
                                        ).toLocaleString()}
                                    </p>
                                </div>
                            </div>
                            <div class="flex items-center gap-2">
                                <Badge variant="secondary"
                                    >{block.transactions} tx</Badge
                                >
                            </div>
                        </div>
                    {/each}
                </div>
            </Card.Content>
        </Card.Root>
    {/if}
</div>
