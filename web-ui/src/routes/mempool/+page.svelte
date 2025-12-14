<script lang="ts">
    import { onMount } from "svelte";
    import { getMempool, type MempoolResponse } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Separator } from "$lib/components/ui/separator";

    let mempool = $state<MempoolResponse | null>(null);
    let loading = $state(true);

    onMount(async () => {
        await loadMempool();
    });

    async function loadMempool() {
        loading = true;
        try {
            mempool = await getMempool();
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }
</script>

<svelte:head>
    <title>Mempool | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-3xl font-bold tracking-tight">Transaction Pool</h1>
            <p class="text-muted-foreground">
                Pending transactions awaiting confirmation
            </p>
        </div>
        <Button
            variant="outline"
            onclick={() => loadMempool()}
            disabled={loading}
        >
            🔄 Refresh
        </Button>
    </div>

    {#if loading}
        <div class="flex items-center justify-center h-64">
            <div
                class="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent"
            ></div>
        </div>
    {:else if mempool}
        <!-- Stats -->
        <div class="grid gap-4 md:grid-cols-2">
            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium"
                        >Pending Transactions</Card.Title
                    >
                    <span class="text-muted-foreground">📬</span>
                </Card.Header>
                <Card.Content>
                    <div class="text-2xl font-bold">
                        {mempool.pending_transactions}
                    </div>
                    <p class="text-xs text-muted-foreground">
                        waiting to be mined
                    </p>
                </Card.Content>
            </Card.Root>

            <Card.Root>
                <Card.Header
                    class="flex flex-row items-center justify-between space-y-0 pb-2"
                >
                    <Card.Title class="text-sm font-medium">Status</Card.Title>
                    <span class="text-muted-foreground">📊</span>
                </Card.Header>
                <Card.Content>
                    {#if mempool.pending_transactions > 0}
                        <Badge>Transactions Pending</Badge>
                    {:else}
                        <Badge variant="secondary">Mempool Empty</Badge>
                    {/if}
                    <p class="text-xs text-muted-foreground mt-2">
                        {mempool.pending_transactions > 0
                            ? "Mine a block to confirm"
                            : "All transactions confirmed"}
                    </p>
                </Card.Content>
            </Card.Root>
        </div>

        <!-- Transaction List -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Pending Transactions</Card.Title>
                <Card.Description
                    >Transactions waiting to be included in a block</Card.Description
                >
            </Card.Header>
            <Card.Content>
                {#if mempool.transactions.length === 0}
                    <div
                        class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                    >
                        <span class="text-4xl mb-4">📬</span>
                        <p class="font-medium">No pending transactions</p>
                        <p class="text-sm mt-1">
                            Transactions will appear here before being mined
                        </p>
                    </div>
                {:else}
                    <div class="space-y-4">
                        {#each mempool.transactions as tx, i}
                            {#if i > 0}
                                <Separator />
                            {/if}
                            <div class="flex items-center justify-between">
                                <div class="space-y-1">
                                    <p class="font-mono text-sm">
                                        {tx.id.slice(0, 32)}...
                                    </p>
                                    <div class="flex items-center gap-2">
                                        {#if tx.is_coinbase}
                                            <Badge variant="outline"
                                                >Coinbase</Badge
                                            >
                                        {/if}
                                        <span
                                            class="text-xs text-muted-foreground"
                                            >{tx.inputs} in → {tx.outputs} out</span
                                        >
                                    </div>
                                </div>
                                <div class="text-right">
                                    <p class="font-bold">{tx.total_output}</p>
                                    <p class="text-xs text-muted-foreground">
                                        coins
                                    </p>
                                </div>
                            </div>
                        {/each}
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>
    {/if}
</div>
