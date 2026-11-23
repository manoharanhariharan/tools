class BusyStore {
  private count = $state(0);
  message = $state("");

  get active(): boolean {
    return this.count > 0;
  }

  get depth(): number {
    return this.count;
  }

  start(message = ""): void {
    this.count += 1;
    if (message) {
      this.message = message;
    }
  }

  stop(): void {
    this.count = Math.max(0, this.count - 1);
    if (this.count === 0) {
      this.message = "";
    }
  }

  reset(): void {
    this.count = 0;
    this.message = "";
  }

  async track<T>(message: string, work: () => Promise<T>): Promise<T> {
    this.start(message);
    try {
      return await work();
    } finally {
      this.stop();
    }
  }
}

export const busy = new BusyStore();
