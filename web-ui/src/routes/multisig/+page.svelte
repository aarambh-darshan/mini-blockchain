<script lang="ts">
    import { onMount } from "svelte";
    import {
        listMultisig,
        createMultisig,
        getMultisigBalance,
        listPendingTx,
        type MultisigWalletInfo,
        type PendingTxInfo,
        type BalanceResponse,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";
    import * as Tabs from "$lib/components/ui/tabs";

    let wallets = $state<MultisigWalletInfo[]>([]);
    let selectedWallet = $state<MultisigWalletInfo | null>(null);
    let selectedBalance = $state<BalanceResponse | null>(null);
    let pendingTxs = $state<PendingTxInfo[]>([]);
    let loading = $state(true);

    // Create state
    let threshold = $state(2);
    let signersInput = $state("");
    let walletLabel = $state("");
    let creating = $state(false);
    let createError = $state("");
    let createResult = $state<MultisigWalletInfo | null>(null);

    onMount(async () => {
        await loadWallets();
    });

    async function loadWallets() {
        loading = true;
        try {
            wallets = await listMultisig();
            if (wallets.length > 0 && !selectedWallet) {
                await selectWallet(wallets[0]);
            }
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function selectWallet(wallet: MultisigWalletInfo) {
        selectedWallet = wallet;
        try {
            selectedBalance = await getMultisigBalance(wallet.address);
            pendingTxs = await listPendingTx(wallet.address);
        } catch (e) {
            console.error(e);
            selectedBalance = null;
            pendingTxs = [];
        }
    }

    async function handleCreate() {
        if (!signersInput.trim()) return;
        creating = true;
        createError = "";
        createResult = null;

        try {
            const signers = signersInput
                .split("\n")
                .map((s) => s.trim())
                .filter((s) => s.length > 0);
            if (signers.length < 2) {
                throw new Error("Need at least 2 signers");
            }
            if (threshold > signers.length) {
                throw new Error(
                    `Threshold (${threshold}) cannot exceed signer count (${signers.length})`,
                );
            }
            createResult = await createMultisig(
                threshold,
                signers,
                walletLabel || undefined,
            );
            await loadWallets();
            // Select the new wallet
            if (createResult) {
                const newWallet = wallets.find(
                    (w) => w.address === createResult!.address,
                );
                if (newWallet) {
                    await selectWallet(newWallet);
                }
            }
        } catch (e: any) {
            createError = e.message;
        } finally {
            creating = false;
        }
    }

    function formatAddress(addr: string): string {
        if (addr.length <= 16) return addr;
        return `${addr.slice(0, 8)}...${addr.slice(-8)}`;
    }

    function formatPubkey(pk: string): string {
        if (pk.length <= 20) return pk;
        return `${pk.slice(0, 10)}...${pk.slice(-10)}`;
    }
</script>

<svelte:head>
    <title>Multisig | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">
            Multi-Signature Wallets
        </h1>
        <p class="text-muted-foreground">
            Create and manage M-of-N threshold signature wallets
        </p>
    </div>

    <Tabs.Root value="wallets" class="w-full">
        <Tabs.List class="grid w-full grid-cols-3">
            <Tabs.Trigger value="wallets"
                >Wallets ({wallets.length})</Tabs.Trigger
            >
            <Tabs.Trigger value="create">Create</Tabs.Trigger>
            <Tabs.Trigger value="pending">Pending Txs</Tabs.Trigger>
        </Tabs.List>

        <!-- Wallets Tab -->
        <Tabs.Content value="wallets" class="mt-4">
            <div class="grid gap-6 lg:grid-cols-2">
                <Card.Root>
                    <Card.Header>
                        <div class="flex items-center justify-between">
                            <div>
                                <Card.Title>✍️ Multisig Wallets</Card.Title>
                                <Card.Description>
                                    {wallets.length} wallet(s) created
                                </Card.Description>
                            </div>
                            <Button
                                variant="outline"
                                size="sm"
                                onclick={() => loadWallets()}
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
                        {:else if wallets.length === 0}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">✍️</span>
                                <p class="font-medium">No multisig wallets</p>
                                <p class="text-sm">
                                    Create one using the Create tab
                                </p>
                            </div>
                        {:else}
                            <div class="space-y-2">
                                {#each wallets as wallet}
                                    <button
                                        type="button"
                                        class="w-full text-left p-4 rounded-lg border transition-colors hover:bg-accent cursor-pointer
                                            {selectedWallet?.address ===
                                        wallet.address
                                            ? 'bg-accent border-primary'
                                            : 'border-transparent'}"
                                        onclick={() => selectWallet(wallet)}
                                    >
                                        <div
                                            class="flex items-center justify-between"
                                        >
                                            <div class="space-y-1">
                                                <p class="font-mono text-sm">
                                                    {formatAddress(
                                                        wallet.address,
                                                    )}
                                                </p>
                                                {#if wallet.label}
                                                    <p
                                                        class="text-xs text-muted-foreground"
                                                    >
                                                        {wallet.label}
                                                    </p>
                                                {/if}
                                            </div>
                                            <Badge variant="outline"
                                                >{wallet.description}</Badge
                                            >
                                        </div>
                                    </button>
                                {/each}
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Wallet Details</Card.Title>
                        <Card.Description
                            >Selected wallet information</Card.Description
                        >
                    </Card.Header>
                    <Card.Content>
                        {#if selectedWallet}
                            <div class="space-y-6">
                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Address
                                    </p>
                                    <code
                                        class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >
                                        {selectedWallet.address}
                                    </code>
                                </div>

                                <div class="grid grid-cols-2 gap-4">
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Type
                                        </p>
                                        <p class="text-xl font-bold">
                                            {selectedWallet.description}
                                        </p>
                                    </div>
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Balance
                                        </p>
                                        <p class="text-xl font-bold font-mono">
                                            {selectedBalance?.balance ?? 0} coins
                                        </p>
                                    </div>
                                </div>

                                <Separator />

                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Signers ({selectedWallet.signer_count})
                                    </p>
                                    <div class="space-y-1">
                                        {#each selectedWallet.signers as signer, i}
                                            <div
                                                class="flex items-center gap-2 text-xs"
                                            >
                                                <Badge variant="secondary"
                                                    >{i + 1}</Badge
                                                >
                                                <code
                                                    class="font-mono text-muted-foreground"
                                                >
                                                    {formatPubkey(signer)}
                                                </code>
                                            </div>
                                        {/each}
                                    </div>
                                </div>

                                <div class="text-xs text-muted-foreground">
                                    Created: {new Date(
                                        selectedWallet.created_at,
                                    ).toLocaleString()}
                                </div>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">✍️</span>
                                <p class="font-medium">No wallet selected</p>
                                <p class="text-sm">
                                    Select a wallet from the list
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
                        <Card.Title>➕ Create Multisig Wallet</Card.Title>
                        <Card.Description>
                            Create a new M-of-N threshold signature wallet
                        </Card.Description>
                    </Card.Header>
                    <Card.Content class="space-y-4">
                        <div class="space-y-2">
                            <Label for="label">Wallet Label (optional)</Label>
                            <Input
                                id="label"
                                bind:value={walletLabel}
                                placeholder="e.g., Team Treasury"
                            />
                        </div>

                        <div class="space-y-2">
                            <Label for="threshold">Threshold (M)</Label>
                            <Input
                                id="threshold"
                                type="number"
                                min="1"
                                bind:value={threshold}
                            />
                            <p class="text-xs text-muted-foreground">
                                Minimum signatures required to spend
                            </p>
                        </div>

                        <div class="space-y-2">
                            <Label for="signers"
                                >Signer Public Keys (one per line)</Label
                            >
                            <textarea
                                id="signers"
                                bind:value={signersInput}
                                class="w-full h-32 p-3 rounded-lg bg-muted font-mono text-xs border border-input resize-none"
                                placeholder="02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5dc
03b31cc9a4c7a6c2b0f3c0e7d2f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4"
                            ></textarea>
                            <p class="text-xs text-muted-foreground">
                                Paste hex-encoded public keys, minimum 2 signers
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
                            onclick={() => handleCreate()}
                            disabled={creating || !signersInput.trim()}
                        >
                            {creating
                                ? "Creating..."
                                : "🔐 Create Multisig Wallet"}
                        </Button>
                    </Card.Footer>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Creation Result</Card.Title>
                        <Card.Description
                            >Your new multisig wallet</Card.Description
                        >
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
                                        <span>✅</span> Wallet Created!
                                    </p>
                                </div>

                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Wallet Address
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
                                            Type
                                        </p>
                                        <p class="text-lg font-bold">
                                            {createResult.description}
                                        </p>
                                    </div>
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Signers
                                        </p>
                                        <p class="text-lg font-bold">
                                            {createResult.signer_count}
                                        </p>
                                    </div>
                                </div>

                                <p class="text-sm text-muted-foreground">
                                    Fund this address to use it. Remember: {createResult.threshold}
                                    signature(s) required to spend.
                                </p>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">🔐</span>
                                <p class="font-medium">No wallet created yet</p>
                                <p class="text-sm">
                                    Fill in the form and click Create
                                </p>
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>
            </div>
        </Tabs.Content>

        <!-- Pending Transactions Tab -->
        <Tabs.Content value="pending" class="mt-4">
            <Card.Root>
                <Card.Header>
                    <div class="flex items-center justify-between">
                        <div>
                            <Card.Title>⏳ Pending Transactions</Card.Title>
                            <Card.Description>
                                Transactions awaiting signatures
                                {#if selectedWallet}
                                    for {formatAddress(selectedWallet.address)}
                                {/if}
                            </Card.Description>
                        </div>
                        {#if selectedWallet}
                            <Button
                                variant="outline"
                                size="sm"
                                onclick={() => selectWallet(selectedWallet!)}
                            >
                                🔄 Refresh
                            </Button>
                        {/if}
                    </div>
                </Card.Header>
                <Card.Content>
                    {#if !selectedWallet}
                        <div
                            class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                        >
                            <span class="text-4xl mb-4">✍️</span>
                            <p class="font-medium">No wallet selected</p>
                            <p class="text-sm">
                                Select a wallet from the Wallets tab first
                            </p>
                        </div>
                    {:else if pendingTxs.length === 0}
                        <div
                            class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                        >
                            <span class="text-4xl mb-4">✅</span>
                            <p class="font-medium">No pending transactions</p>
                            <p class="text-sm">
                                All transactions have been processed
                            </p>
                        </div>
                    {:else}
                        <div class="space-y-4">
                            {#each pendingTxs as tx, i}
                                {#if i > 0}
                                    <Separator />
                                {/if}
                                <div class="p-4 rounded-lg border">
                                    <div
                                        class="flex items-center justify-between mb-3"
                                    >
                                        <code class="text-xs font-mono"
                                            >{tx.id}</code
                                        >
                                        <Badge
                                            variant={tx.status === "Ready"
                                                ? "default"
                                                : "secondary"}
                                        >
                                            {tx.status}
                                        </Badge>
                                    </div>

                                    <div class="grid grid-cols-2 gap-4 text-sm">
                                        <div>
                                            <p class="text-muted-foreground">
                                                To
                                            </p>
                                            <p class="font-mono">
                                                {formatAddress(tx.to_address)}
                                            </p>
                                        </div>
                                        <div>
                                            <p class="text-muted-foreground">
                                                Amount
                                            </p>
                                            <p class="font-bold">
                                                {tx.amount} coins
                                            </p>
                                        </div>
                                    </div>

                                    <div class="mt-3 flex items-center gap-2">
                                        <div
                                            class="flex-1 bg-muted rounded-full h-2"
                                        >
                                            <div
                                                class="bg-primary h-2 rounded-full transition-all"
                                                style="width: {(tx.signatures_collected /
                                                    tx.signatures_required) *
                                                    100}%"
                                            ></div>
                                        </div>
                                        <span
                                            class="text-xs text-muted-foreground"
                                        >
                                            {tx.signatures_collected}/{tx.signatures_required}
                                            signatures
                                        </span>
                                    </div>

                                    <p
                                        class="text-xs text-muted-foreground mt-2"
                                    >
                                        Created: {new Date(
                                            tx.created_at,
                                        ).toLocaleString()}
                                    </p>
                                </div>
                            {/each}
                        </div>
                    {/if}
                </Card.Content>
            </Card.Root>
        </Tabs.Content>
    </Tabs.Root>
</div>
