// WebSocket client for real-time blockchain updates

import { writable, type Writable } from 'svelte/store';
import type { BlockInfo } from './api';

// WebSocket event types matching backend
export interface WsEventBlockMined {
    type: 'BlockMined';
    data: {
        block: BlockInfo;
        reward: number;
    };
}

export interface WsEventTransactionAdded {
    type: 'TransactionAdded';
    data: {
        transaction: {
            id: string;
            is_coinbase: boolean;
            inputs: number;
            outputs: number;
            total_output: number;
        };
    };
}

export interface WsEventChainUpdated {
    type: 'ChainUpdated';
    data: {
        height: number;
        latest_hash: string;
        total_transactions: number;
    };
}

export interface WsEventConnected {
    type: 'Connected';
    data: {
        message: string;
    };
}

export interface WsEventPing {
    type: 'Ping';
}

export type WsEvent = WsEventBlockMined | WsEventTransactionAdded | WsEventChainUpdated | WsEventConnected | WsEventPing;

// Connection status
export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected';

// Stores
export const wsStatus: Writable<ConnectionStatus> = writable('disconnected');
export const latestBlock: Writable<BlockInfo | null> = writable(null);
export const latestEvent: Writable<WsEvent | null> = writable(null);

// WebSocket instance
let ws: WebSocket | null = null;
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_DELAY = 30000; // 30 seconds
const BASE_RECONNECT_DELAY = 1000; // 1 second

/**
 * Get WebSocket URL based on current location
 */
function getWsUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws`;
}

/**
 * Calculate exponential backoff delay
 */
function getReconnectDelay(): number {
    const delay = Math.min(
        BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttempts),
        MAX_RECONNECT_DELAY
    );
    return delay;
}

/**
 * Handle incoming WebSocket message
 */
function handleMessage(event: MessageEvent): void {
    try {
        const data: WsEvent = JSON.parse(event.data);
        latestEvent.set(data);

        switch (data.type) {
            case 'BlockMined':
                latestBlock.set(data.data.block);
                console.log('🧱 New block mined:', data.data.block.index);
                break;
            case 'Connected':
                console.log('✅ WebSocket:', data.data.message);
                break;
            case 'ChainUpdated':
                console.log('🔄 Chain updated:', data.data.height);
                break;
            case 'TransactionAdded':
                console.log('📝 Transaction added:', data.data.transaction.id);
                break;
            case 'Ping':
                // Heartbeat, no action needed
                break;
        }
    } catch (e) {
        console.error('Failed to parse WebSocket message:', e);
    }
}

/**
 * Connect to WebSocket server
 */
export function connectWebSocket(): void {
    // Don't connect if already connected or connecting
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
        return;
    }

    // Clear any pending reconnect
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
    }

    wsStatus.set('connecting');

    try {
        ws = new WebSocket(getWsUrl());

        ws.onopen = () => {
            wsStatus.set('connected');
            reconnectAttempts = 0;
            console.log('🔌 WebSocket connected');
        };

        ws.onmessage = handleMessage;

        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };

        ws.onclose = (event) => {
            ws = null;
            wsStatus.set('disconnected');

            // Schedule reconnect unless it was a clean close
            if (!event.wasClean) {
                const delay = getReconnectDelay();
                reconnectAttempts++;
                console.log(`🔄 WebSocket closed, reconnecting in ${delay}ms...`);
                reconnectTimeout = setTimeout(connectWebSocket, delay);
            } else {
                console.log('📴 WebSocket closed cleanly');
            }
        };
    } catch (e) {
        console.error('Failed to create WebSocket:', e);
        wsStatus.set('disconnected');

        // Schedule reconnect
        const delay = getReconnectDelay();
        reconnectAttempts++;
        reconnectTimeout = setTimeout(connectWebSocket, delay);
    }
}

/**
 * Disconnect WebSocket
 */
export function disconnectWebSocket(): void {
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
    }

    if (ws) {
        ws.close(1000, 'Client disconnect');
        ws = null;
    }

    wsStatus.set('disconnected');
    reconnectAttempts = 0;
}

/**
 * Check if WebSocket is connected
 */
export function isConnected(): boolean {
    return ws !== null && ws.readyState === WebSocket.OPEN;
}
