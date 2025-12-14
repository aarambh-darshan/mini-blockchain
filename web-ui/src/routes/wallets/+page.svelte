<script lang="ts">
    import { onMount } from "svelte";
    import {
        getWallets,
        createWallet,
        getBalance,
        type WalletResponse,
        type BalanceResponse,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";

    let wallets = $state<WalletResponse[]>([]);
    let selectedWallet = $state<BalanceResponse | null>(null);
    let newLabel = $state("");
    let loading = $state(true);
    let creating = $state(false);

    onMount(async () => {
        await loadWallets();
    });

    async function loadWallets() {
        try {
            wallets = await getWallets();
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function handleCreateWallet() {
        creating = true;
        try {
            await createWallet(newLabel || undefined);
            newLabel = "";
            await loadWallets();
        } catch (e) {
            console.error(e);
        } finally {
            creating = false;
        }
    }

    async function selectWallet(address: string) {
        selectedWallet = await getBalance(address);
    }
</script>

<svelte:head>
    <title>Wallets | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Wallets</h1>
        <p class="text-muted-foreground">Manage your blockchain wallets</p>
    </div>

    <div class="grid gap-6 lg:grid-cols-3">
        <!-- Create Wallet -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Create Wallet</Card.Title>
                <Card.Description
                    >Generate a new wallet with a unique address</Card.Description
                >
            </Card.Header>
            <Card.Content class="space-y-4">
                <div class="space-y-2">
                    <Label for="label">Label (optional)</Label>
                    <Input
                        id="label"
                        bind:value={newLabel}
                        placeholder="My Wallet"
                    />
                </div>
            </Card.Content>
            <Card.Footer>
                <Button
                    class="w-full"
                    onclick={() => handleCreateWallet()}
                    disabled={creating}
                >
                    {creating ? "Creating..." : "+ Create Wallet"}
                </Button>
            </Card.Footer>
        </Card.Root>

        <!-- Wallet List -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Your Wallets</Card.Title>
                <Card.Description
                    >{wallets.length} wallet(s) found</Card.Description
                >
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
                        class="flex flex-col items-center justify-center h-32 text-muted-foreground"
                    >
                        <span class="text-2xl mb-2">👛</span>
                        <p class="text-sm">No wallets yet</p>
                    </div>
                {:else}
                    <div class="space-y-2 max-h-[300px] overflow-y-auto">
                        {#each wallets as wallet}
                            <button
                                type="button"
                                class="w-full text-left p-3 rounded-lg border transition-colors hover:bg-accent
                  {selectedWallet?.address === wallet.address
                                    ? 'bg-accent border-primary'
                                    : 'border-transparent'}"
                                onclick={() => selectWallet(wallet.address)}
                            >
                                <p class="font-mono text-xs truncate">
                                    {wallet.address}
                                </p>
                                {#if wallet.label}
                                    <Badge variant="outline" class="mt-1"
                                        >{wallet.label}</Badge
                                    >
                                {/if}
                            </button>
                        {/each}
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>

        <!-- Wallet Balance -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Wallet Details</Card.Title>
                <Card.Description>View balance and UTXOs</Card.Description>
            </Card.Header>
            <Card.Content>
                {#if selectedWallet}
                    <div class="space-y-6">
                        <div class="space-y-2">
                            <p class="text-sm text-muted-foreground">Address</p>
                            <code
                                class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                >{selectedWallet.address}</code
                            >
                        </div>

                        <Separator />

                        <div class="text-center">
                            <p class="text-sm text-muted-foreground mb-1">
                                Balance
                            </p>
                            <p class="text-4xl font-bold">
                                {selectedWallet.balance}
                            </p>
                            <p class="text-sm text-muted-foreground">coins</p>
                        </div>

                        <div class="flex justify-center">
                            <Badge variant="secondary"
                                >{selectedWallet.utxo_count} UTXOs</Badge
                            >
                        </div>
                    </div>
                {:else}
                    <div
                        class="flex flex-col items-center justify-center h-40 text-muted-foreground"
                    >
                        <span class="text-2xl mb-2">💰</span>
                        <p class="text-sm">Select a wallet</p>
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>
    </div>
</div>
