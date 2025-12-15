<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/stores";
    import { search, type SearchResult } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Badge } from "$lib/components/ui/badge";
    import * as Tabs from "$lib/components/ui/tabs";

    let results = $state<SearchResult | null>(null);
    let loading = $state(true);
    let error = $state<string | null>(null);

    $effect(() => {
        const query = $page.url.searchParams.get("q");
        if (query) {
            performSearch(query);
        } else {
            loading = false;
        }
    });

    async function performSearch(query: string) {
        loading = true;
        error = null;
        try {
            results = await search(query);
        } catch (e) {
            error = e instanceof Error ? e.message : "Search failed";
        } finally {
            loading = false;
        }
    }

    function totalResults(): number {
        if (!results) return 0;
        return (
            results.blocks.length +
            results.transactions.length +
            results.wallets.length +
            results.contracts.length +
            results.tokens.length +
            results.multisig.length
        );
    }
</script>

<svelte:head>
    <title>Search Results | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">🔍 Search Results</h1>
        {#if results}
            <p class="text-muted-foreground">
                Found {totalResults()} results for "{results.query}"
            </p>
        {/if}
    </div>

    {#if loading}
        <Card.Root>
            <Card.Content class="flex items-center justify-center h-32">
                <div
                    class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
                ></div>
            </Card.Content>
        </Card.Root>
    {:else if error}
        <Card.Root>
            <Card.Content class="py-6">
                <div class="text-center text-destructive">
                    <p class="text-lg font-medium">Search Error</p>
                    <p class="text-sm">{error}</p>
                </div>
            </Card.Content>
        </Card.Root>
    {:else if !results || totalResults() === 0}
        <Card.Root>
            <Card.Content class="py-12">
                <div class="text-center text-muted-foreground">
                    <span class="text-4xl mb-4 block">🔍</span>
                    <p class="text-lg">No results found</p>
                    <p class="text-sm">
                        Try searching for a block height, hash, address, or
                        token name
                    </p>
                </div>
            </Card.Content>
        </Card.Root>
    {:else}
        <Tabs.Root value="all" class="w-full">
            <Tabs.List class="grid w-full grid-cols-6">
                <Tabs.Trigger value="all">All ({totalResults()})</Tabs.Trigger>
                <Tabs.Trigger value="blocks"
                    >Blocks ({results.blocks.length})</Tabs.Trigger
                >
                <Tabs.Trigger value="transactions"
                    >Txns ({results.transactions.length})</Tabs.Trigger
                >
                <Tabs.Trigger value="addresses"
                    >Addresses ({results.wallets.length +
                        results.multisig.length})</Tabs.Trigger
                >
                <Tabs.Trigger value="contracts"
                    >Contracts ({results.contracts.length})</Tabs.Trigger
                >
                <Tabs.Trigger value="tokens"
                    >Tokens ({results.tokens.length})</Tabs.Trigger
                >
            </Tabs.List>

            <!-- All Results -->
            <Tabs.Content value="all" class="space-y-4 mt-4">
                {#if results.blocks.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg">🧱 Blocks</Card.Title>
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.blocks as block}
                                    <a
                                        href="/blocks"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div class="flex items-center gap-3">
                                            <Badge variant="outline"
                                                >#{block.index}</Badge
                                            >
                                            <code
                                                class="text-xs text-muted-foreground"
                                                >{block.hash.slice(
                                                    0,
                                                    24,
                                                )}...</code
                                            >
                                        </div>
                                        <Badge variant="secondary"
                                            >{block.transactions} txs</Badge
                                        >
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}

                {#if results.transactions.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg"
                                >📝 Transactions</Card.Title
                            >
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.transactions as tx}
                                    <div
                                        class="flex items-center justify-between p-3 rounded-lg border"
                                    >
                                        <div class="flex items-center gap-3">
                                            <code class="text-xs"
                                                >{tx.id.slice(0, 24)}...</code
                                            >
                                            {#if tx.is_coinbase}
                                                <Badge variant="default"
                                                    >Coinbase</Badge
                                                >
                                            {/if}
                                        </div>
                                        <span
                                            class="text-sm text-muted-foreground"
                                            >{tx.total_output} coins</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}

                {#if results.wallets.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg">👛 Wallets</Card.Title>
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.wallets as wallet}
                                    <a
                                        href="/wallets"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <code class="text-sm"
                                            >{wallet.address}</code
                                        >
                                        {#if wallet.label}
                                            <Badge variant="secondary"
                                                >{wallet.label}</Badge
                                            >
                                        {/if}
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}

                {#if results.multisig.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg"
                                >✍️ Multisig Wallets</Card.Title
                            >
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.multisig as ms}
                                    <a
                                        href="/multisig"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div class="flex items-center gap-3">
                                            <code class="text-sm"
                                                >{ms.address.slice(
                                                    0,
                                                    20,
                                                )}...</code
                                            >
                                            <Badge variant="outline"
                                                >{ms.description}</Badge
                                            >
                                        </div>
                                        {#if ms.label}
                                            <Badge variant="secondary"
                                                >{ms.label}</Badge
                                            >
                                        {/if}
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}

                {#if results.contracts.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg">📜 Contracts</Card.Title
                            >
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.contracts as contract}
                                    <a
                                        href="/contracts"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <code class="text-sm"
                                            >{contract.address}</code
                                        >
                                        <Badge variant="secondary"
                                            >{contract.code_size} bytes</Badge
                                        >
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}

                {#if results.tokens.length > 0}
                    <Card.Root>
                        <Card.Header class="pb-3">
                            <Card.Title class="text-lg">🪙 Tokens</Card.Title>
                        </Card.Header>
                        <Card.Content>
                            <div class="space-y-2">
                                {#each results.tokens as token}
                                    <a
                                        href="/tokens"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div class="flex items-center gap-3">
                                            <span class="font-medium"
                                                >{token.name}</span
                                            >
                                            <Badge variant="outline"
                                                >{token.symbol}</Badge
                                            >
                                        </div>
                                        <span
                                            class="text-sm text-muted-foreground"
                                            >{token.holder_count} holders</span
                                        >
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}
            </Tabs.Content>

            <!-- Blocks Tab -->
            <Tabs.Content value="blocks" class="mt-4">
                {#if results.blocks.length === 0}
                    <Card.Root>
                        <Card.Content
                            class="py-8 text-center text-muted-foreground"
                        >
                            No blocks found
                        </Card.Content>
                    </Card.Root>
                {:else}
                    <Card.Root>
                        <Card.Content class="pt-6">
                            <div class="space-y-2">
                                {#each results.blocks as block}
                                    <a
                                        href="/blocks"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div class="flex items-center gap-3">
                                            <Badge variant="outline"
                                                >#{block.index}</Badge
                                            >
                                            <code
                                                class="text-xs text-muted-foreground"
                                                >{block.hash}</code
                                            >
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <Badge variant="secondary"
                                                >{block.transactions} txs</Badge
                                            >
                                            <span
                                                class="text-xs text-muted-foreground"
                                                >{new Date(
                                                    block.timestamp,
                                                ).toLocaleString()}</span
                                            >
                                        </div>
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}
            </Tabs.Content>

            <!-- Transactions Tab -->
            <Tabs.Content value="transactions" class="mt-4">
                {#if results.transactions.length === 0}
                    <Card.Root>
                        <Card.Content
                            class="py-8 text-center text-muted-foreground"
                        >
                            No transactions found
                        </Card.Content>
                    </Card.Root>
                {:else}
                    <Card.Root>
                        <Card.Content class="pt-6">
                            <div class="space-y-2">
                                {#each results.transactions as tx}
                                    <div
                                        class="flex items-center justify-between p-3 rounded-lg border"
                                    >
                                        <div class="flex items-center gap-3">
                                            <code class="text-sm">{tx.id}</code>
                                            {#if tx.is_coinbase}
                                                <Badge variant="default"
                                                    >Coinbase</Badge
                                                >
                                            {/if}
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <span class="text-sm"
                                                >{tx.inputs} inputs → {tx.outputs}
                                                outputs</span
                                            >
                                            <Badge variant="secondary"
                                                >{tx.total_output} coins</Badge
                                            >
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}
            </Tabs.Content>

            <!-- Addresses Tab -->
            <Tabs.Content value="addresses" class="mt-4 space-y-4">
                {#if results.wallets.length === 0 && results.multisig.length === 0}
                    <Card.Root>
                        <Card.Content
                            class="py-8 text-center text-muted-foreground"
                        >
                            No addresses found
                        </Card.Content>
                    </Card.Root>
                {:else}
                    {#if results.wallets.length > 0}
                        <Card.Root>
                            <Card.Header class="pb-3">
                                <Card.Title class="text-lg"
                                    >👛 Wallets</Card.Title
                                >
                            </Card.Header>
                            <Card.Content>
                                <div class="space-y-2">
                                    {#each results.wallets as wallet}
                                        <a
                                            href="/wallets"
                                            class="flex items-center p-3 rounded-lg border hover:bg-accent transition-colors"
                                        >
                                            <code class="text-sm"
                                                >{wallet.address}</code
                                            >
                                        </a>
                                    {/each}
                                </div>
                            </Card.Content>
                        </Card.Root>
                    {/if}
                    {#if results.multisig.length > 0}
                        <Card.Root>
                            <Card.Header class="pb-3">
                                <Card.Title class="text-lg"
                                    >✍️ Multisig</Card.Title
                                >
                            </Card.Header>
                            <Card.Content>
                                <div class="space-y-2">
                                    {#each results.multisig as ms}
                                        <a
                                            href="/multisig"
                                            class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                        >
                                            <code class="text-sm"
                                                >{ms.address}</code
                                            >
                                            <Badge variant="outline"
                                                >{ms.description}</Badge
                                            >
                                        </a>
                                    {/each}
                                </div>
                            </Card.Content>
                        </Card.Root>
                    {/if}
                {/if}
            </Tabs.Content>

            <!-- Contracts Tab -->
            <Tabs.Content value="contracts" class="mt-4">
                {#if results.contracts.length === 0}
                    <Card.Root>
                        <Card.Content
                            class="py-8 text-center text-muted-foreground"
                        >
                            No contracts found
                        </Card.Content>
                    </Card.Root>
                {:else}
                    <Card.Root>
                        <Card.Content class="pt-6">
                            <div class="space-y-2">
                                {#each results.contracts as contract}
                                    <a
                                        href="/contracts"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div>
                                            <code class="text-sm"
                                                >{contract.address}</code
                                            >
                                            <p
                                                class="text-xs text-muted-foreground"
                                            >
                                                Deployed by {contract.deployer} at
                                                block {contract.deployed_at}
                                            </p>
                                        </div>
                                        <Badge variant="secondary"
                                            >{contract.code_size} bytes</Badge
                                        >
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}
            </Tabs.Content>

            <!-- Tokens Tab -->
            <Tabs.Content value="tokens" class="mt-4">
                {#if results.tokens.length === 0}
                    <Card.Root>
                        <Card.Content
                            class="py-8 text-center text-muted-foreground"
                        >
                            No tokens found
                        </Card.Content>
                    </Card.Root>
                {:else}
                    <Card.Root>
                        <Card.Content class="pt-6">
                            <div class="space-y-2">
                                {#each results.tokens as token}
                                    <a
                                        href="/tokens"
                                        class="flex items-center justify-between p-3 rounded-lg border hover:bg-accent transition-colors"
                                    >
                                        <div>
                                            <div
                                                class="flex items-center gap-2"
                                            >
                                                <span class="font-medium"
                                                    >{token.name}</span
                                                >
                                                <Badge variant="outline"
                                                    >{token.symbol}</Badge
                                                >
                                            </div>
                                            <code
                                                class="text-xs text-muted-foreground"
                                                >{token.address}</code
                                            >
                                        </div>
                                        <div class="text-right">
                                            <p class="text-sm">
                                                {token.total_supply} supply
                                            </p>
                                            <p
                                                class="text-xs text-muted-foreground"
                                            >
                                                {token.holder_count} holders
                                            </p>
                                        </div>
                                    </a>
                                {/each}
                            </div>
                        </Card.Content>
                    </Card.Root>
                {/if}
            </Tabs.Content>
        </Tabs.Root>
    {/if}
</div>
