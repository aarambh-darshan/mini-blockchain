<script lang="ts">
    import { onMount } from "svelte";
    import {
        listTokens,
        createToken,
        getTokenBalance,
        transferTokens,
        type TokenInfo,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";
    import * as Tabs from "$lib/components/ui/tabs";

    let tokens = $state<TokenInfo[]>([]);
    let selectedToken = $state<TokenInfo | null>(null);
    let loading = $state(true);

    // Create state
    let tokenName = $state("");
    let tokenSymbol = $state("");
    let tokenDecimals = $state(18);
    let tokenSupply = $state("1000000");
    let tokenCreator = $state("");
    let creating = $state(false);
    let createError = $state("");
    let createResult = $state<TokenInfo | null>(null);

    // Balance check state
    let balanceAddress = $state("");
    let checkedBalance = $state<string | null>(null);
    let balanceError = $state("");

    // Transfer state
    let transferFrom = $state("");
    let transferTo = $state("");
    let transferAmount = $state("");
    let transferring = $state(false);
    let transferError = $state("");
    let transferSuccess = $state(false);

    onMount(async () => {
        await loadTokens();
    });

    async function loadTokens() {
        loading = true;
        try {
            tokens = await listTokens();
            if (tokens.length > 0 && !selectedToken) {
                selectedToken = tokens[0];
            }
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    function selectToken(token: TokenInfo) {
        selectedToken = token;
        checkedBalance = null;
        balanceError = "";
        transferSuccess = false;
        transferError = "";
    }

    async function handleCreate() {
        if (!tokenName.trim() || !tokenSymbol.trim() || !tokenCreator.trim())
            return;
        creating = true;
        createError = "";
        createResult = null;

        try {
            createResult = await createToken(
                tokenName,
                tokenSymbol,
                tokenDecimals,
                tokenSupply,
                tokenCreator,
            );
            await loadTokens();
            // Select new token
            if (createResult) {
                const newToken = tokens.find(
                    (t) => t.address === createResult!.address,
                );
                if (newToken) selectToken(newToken);
            }
        } catch (e: any) {
            createError = e.message;
        } finally {
            creating = false;
        }
    }

    async function handleCheckBalance() {
        if (!selectedToken || !balanceAddress.trim()) return;
        balanceError = "";
        checkedBalance = null;

        try {
            const result = await getTokenBalance(
                selectedToken.address,
                balanceAddress,
            );
            checkedBalance = result.balance;
        } catch (e: any) {
            balanceError = e.message;
        }
    }

    async function handleTransfer() {
        if (!selectedToken || !transferFrom || !transferTo || !transferAmount)
            return;
        transferring = true;
        transferError = "";
        transferSuccess = false;

        try {
            await transferTokens(
                selectedToken.address,
                transferFrom,
                transferTo,
                transferAmount,
            );
            transferSuccess = true;
            await loadTokens();
        } catch (e: any) {
            transferError = e.message;
        } finally {
            transferring = false;
        }
    }

    function formatAddress(addr: string): string {
        if (addr.length <= 16) return addr;
        return `${addr.slice(0, 8)}...${addr.slice(-8)}`;
    }

    function formatSupply(supply: string, decimals: number): string {
        const num = BigInt(supply);
        const divisor = BigInt(10 ** decimals);
        const whole = num / divisor;
        return whole.toLocaleString();
    }
</script>

<svelte:head>
    <title>Tokens | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Tokens</h1>
        <p class="text-muted-foreground">
            Create and manage ERC-20 style fungible tokens
        </p>
    </div>

    <Tabs.Root value="tokens" class="w-full">
        <Tabs.List class="grid w-full grid-cols-3">
            <Tabs.Trigger value="tokens">Tokens ({tokens.length})</Tabs.Trigger>
            <Tabs.Trigger value="create">Create</Tabs.Trigger>
            <Tabs.Trigger value="transfer">Transfer</Tabs.Trigger>
        </Tabs.List>

        <!-- Tokens Tab -->
        <Tabs.Content value="tokens" class="mt-4">
            <div class="grid gap-6 lg:grid-cols-2">
                <Card.Root>
                    <Card.Header>
                        <div class="flex items-center justify-between">
                            <div>
                                <Card.Title>🪙 Token List</Card.Title>
                                <Card.Description
                                    >{tokens.length} token(s) created</Card.Description
                                >
                            </div>
                            <Button
                                variant="outline"
                                size="sm"
                                onclick={() => loadTokens()}
                            >
                                🔄 Refresh
                            </Button>
                        </div>
                    </Card.Header>
                    <Card.Content>
                        {#if loading}
                            <div class="flex items-center justify-center h-32">
                                <div
                                    class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
                                ></div>
                            </div>
                        {:else if tokens.length === 0}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">🪙</span>
                                <p class="font-medium">No tokens created</p>
                                <p class="text-sm">
                                    Create one using the Create tab
                                </p>
                            </div>
                        {:else}
                            <div class="space-y-2">
                                {#each tokens as token}
                                    <button
                                        type="button"
                                        class="w-full text-left p-4 rounded-lg border transition-colors hover:bg-accent cursor-pointer
                                            {selectedToken?.address ===
                                        token.address
                                            ? 'bg-accent border-primary'
                                            : 'border-transparent'}"
                                        onclick={() => selectToken(token)}
                                    >
                                        <div
                                            class="flex items-center justify-between"
                                        >
                                            <div class="space-y-1">
                                                <div
                                                    class="flex items-center gap-2"
                                                >
                                                    <span class="font-bold"
                                                        >{token.name}</span
                                                    >
                                                    <Badge variant="outline"
                                                        >{token.symbol}</Badge
                                                    >
                                                </div>
                                                <p
                                                    class="text-xs text-muted-foreground font-mono"
                                                >
                                                    {formatAddress(
                                                        token.address,
                                                    )}
                                                </p>
                                            </div>
                                            <div class="text-right">
                                                <p class="text-sm font-mono">
                                                    {formatSupply(
                                                        token.total_supply,
                                                        token.decimals,
                                                    )}
                                                </p>
                                                <p
                                                    class="text-xs text-muted-foreground"
                                                >
                                                    {token.holder_count} holder(s)
                                                </p>
                                            </div>
                                        </div>
                                    </button>
                                {/each}
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Token Details</Card.Title>
                        <Card.Description
                            >Selected token information</Card.Description
                        >
                    </Card.Header>
                    <Card.Content>
                        {#if selectedToken}
                            <div class="space-y-6">
                                <div class="flex items-center gap-3">
                                    <span class="text-3xl">🪙</span>
                                    <div>
                                        <h3 class="text-xl font-bold">
                                            {selectedToken.name}
                                        </h3>
                                        <Badge>{selectedToken.symbol}</Badge>
                                    </div>
                                </div>

                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Contract Address
                                    </p>
                                    <code
                                        class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >
                                        {selectedToken.address}
                                    </code>
                                </div>

                                <div class="grid grid-cols-2 gap-4">
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Total Supply
                                        </p>
                                        <p class="text-lg font-bold font-mono">
                                            {formatSupply(
                                                selectedToken.total_supply,
                                                selectedToken.decimals,
                                            )}
                                        </p>
                                    </div>
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Decimals
                                        </p>
                                        <p class="text-lg font-bold">
                                            {selectedToken.decimals}
                                        </p>
                                    </div>
                                </div>

                                <Separator />

                                <div class="space-y-3">
                                    <Label>Check Balance</Label>
                                    <div class="flex gap-2">
                                        <Input
                                            bind:value={balanceAddress}
                                            placeholder="Enter wallet address"
                                            class="flex-1"
                                        />
                                        <Button onclick={handleCheckBalance}
                                            >Check</Button
                                        >
                                    </div>
                                    {#if checkedBalance !== null}
                                        <div class="rounded-lg bg-muted p-3">
                                            <p
                                                class="text-sm text-muted-foreground"
                                            >
                                                Balance
                                            </p>
                                            <p
                                                class="text-xl font-bold font-mono"
                                            >
                                                {checkedBalance}
                                                {selectedToken.symbol}
                                            </p>
                                        </div>
                                    {/if}
                                    {#if balanceError}
                                        <p class="text-sm text-destructive">
                                            {balanceError}
                                        </p>
                                    {/if}
                                </div>

                                <div class="text-xs text-muted-foreground">
                                    Creator: {formatAddress(
                                        selectedToken.creator,
                                    )} • Block #{selectedToken.created_at_block}
                                </div>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">🪙</span>
                                <p class="font-medium">No token selected</p>
                                <p class="text-sm">
                                    Select a token from the list
                                </p>
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>
            </div>
        </Tabs.Content>

        <!-- Create Tab -->
        <Tabs.Content value="create" class="mt-4">
            <div class="grid gap-6 lg:grid-cols-2">
                <Card.Root>
                    <Card.Header>
                        <Card.Title>➕ Create Token</Card.Title>
                        <Card.Description>
                            Deploy a new ERC-20 style token
                        </Card.Description>
                    </Card.Header>
                    <Card.Content class="space-y-4">
                        <div class="grid grid-cols-2 gap-4">
                            <div class="space-y-2">
                                <Label for="name">Token Name</Label>
                                <Input
                                    id="name"
                                    bind:value={tokenName}
                                    placeholder="My Token"
                                />
                            </div>
                            <div class="space-y-2">
                                <Label for="symbol">Symbol</Label>
                                <Input
                                    id="symbol"
                                    bind:value={tokenSymbol}
                                    placeholder="MTK"
                                    maxlength={10}
                                />
                            </div>
                        </div>

                        <div class="grid grid-cols-2 gap-4">
                            <div class="space-y-2">
                                <Label for="supply">Total Supply</Label>
                                <Input
                                    id="supply"
                                    bind:value={tokenSupply}
                                    placeholder="1000000"
                                />
                            </div>
                            <div class="space-y-2">
                                <Label for="decimals">Decimals</Label>
                                <Input
                                    id="decimals"
                                    type="number"
                                    min="0"
                                    max="18"
                                    bind:value={tokenDecimals}
                                />
                            </div>
                        </div>

                        <div class="space-y-2">
                            <Label for="creator">Creator Address</Label>
                            <Input
                                id="creator"
                                bind:value={tokenCreator}
                                placeholder="1ABC..."
                            />
                            <p class="text-xs text-muted-foreground">
                                All tokens will be allocated to this address
                            </p>
                        </div>

                        {#if createError}
                            <p class="text-sm text-destructive">
                                {createError}
                            </p>
                        {/if}
                    </Card.Content>
                    <Card.Footer>
                        <Button
                            class="w-full"
                            onclick={handleCreate}
                            disabled={creating ||
                                !tokenName.trim() ||
                                !tokenSymbol.trim() ||
                                !tokenCreator.trim()}
                        >
                            {creating ? "Creating..." : "🪙 Create Token"}
                        </Button>
                    </Card.Footer>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Creation Result</Card.Title>
                        <Card.Description>Your new token</Card.Description>
                    </Card.Header>
                    <Card.Content>
                        {#if createResult}
                            <div class="space-y-6">
                                <div
                                    class="rounded-lg border border-green-500/50 bg-green-500/10 p-4"
                                >
                                    <p
                                        class="text-green-500 font-medium flex items-center gap-2"
                                    >
                                        <span>✅</span> Token Created!
                                    </p>
                                </div>

                                <div class="flex items-center gap-3">
                                    <span class="text-3xl">🪙</span>
                                    <div>
                                        <h3 class="font-bold">
                                            {createResult.name}
                                        </h3>
                                        <Badge>{createResult.symbol}</Badge>
                                    </div>
                                </div>

                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Contract Address
                                    </p>
                                    <code
                                        class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >
                                        {createResult.address}
                                    </code>
                                </div>

                                <div class="grid grid-cols-2 gap-4">
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Supply
                                        </p>
                                        <p class="font-bold font-mono">
                                            {formatSupply(
                                                createResult.total_supply,
                                                createResult.decimals,
                                            )}
                                        </p>
                                    </div>
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Decimals
                                        </p>
                                        <p class="font-bold">
                                            {createResult.decimals}
                                        </p>
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">🪙</span>
                                <p class="font-medium">No token created yet</p>
                                <p class="text-sm">
                                    Fill in the form and click Create
                                </p>
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>
            </div>
        </Tabs.Content>

        <!-- Transfer Tab -->
        <Tabs.Content value="transfer" class="mt-4">
            <Card.Root class="max-w-2xl">
                <Card.Header>
                    <Card.Title>💸 Transfer Tokens</Card.Title>
                    <Card.Description>
                        Transfer tokens between addresses
                    </Card.Description>
                </Card.Header>
                <Card.Content class="space-y-4">
                    {#if !selectedToken}
                        <div
                            class="flex flex-col items-center justify-center h-32 text-muted-foreground"
                        >
                            <p>Select a token from the Tokens tab first</p>
                        </div>
                    {:else}
                        <div
                            class="flex items-center gap-2 p-3 rounded-lg bg-muted"
                        >
                            <span>🪙</span>
                            <span class="font-bold">{selectedToken.name}</span>
                            <Badge variant="outline"
                                >{selectedToken.symbol}</Badge
                            >
                        </div>

                        <div class="space-y-2">
                            <Label for="from">From Address</Label>
                            <Input
                                id="from"
                                bind:value={transferFrom}
                                placeholder="Sender address"
                            />
                        </div>

                        <div class="space-y-2">
                            <Label for="to">To Address</Label>
                            <Input
                                id="to"
                                bind:value={transferTo}
                                placeholder="Recipient address"
                            />
                        </div>

                        <div class="space-y-2">
                            <Label for="amount">Amount</Label>
                            <Input
                                id="amount"
                                bind:value={transferAmount}
                                placeholder="Amount to transfer"
                            />
                        </div>

                        {#if transferError}
                            <div
                                class="rounded-lg border border-destructive/50 bg-destructive/10 p-3"
                            >
                                <p class="text-destructive text-sm">
                                    {transferError}
                                </p>
                            </div>
                        {/if}

                        {#if transferSuccess}
                            <div
                                class="rounded-lg border border-green-500/50 bg-green-500/10 p-3"
                            >
                                <p class="text-green-500 font-medium">
                                    ✅ Transfer successful!
                                </p>
                            </div>
                        {/if}
                    {/if}
                </Card.Content>
                {#if selectedToken}
                    <Card.Footer>
                        <Button
                            class="w-full"
                            onclick={handleTransfer}
                            disabled={transferring ||
                                !transferFrom ||
                                !transferTo ||
                                !transferAmount}
                        >
                            {transferring ? "Transferring..." : "💸 Transfer"}
                        </Button>
                    </Card.Footer>
                {/if}
            </Card.Root>
        </Tabs.Content>
    </Tabs.Root>
</div>
