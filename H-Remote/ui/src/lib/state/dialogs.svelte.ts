export type DialogButtonVariant = "primary" | "secondary" | "danger";

export interface DialogButton {
  value: string;
  label: string;
  variant?: DialogButtonVariant;
}

export type DialogRequest =
  | {
      kind: "message";
      title: string;
      message: string;
      variant: "info" | "warning" | "error";
      buttons: DialogButton[];
    }
  | {
      kind: "input";
      title: string;
      message: string;
      label: string;
      value: string;
      placeholder: string;
      submitLabel: string;
    }
  | {
      kind: "password";
      title: string;
      message: string;
      label: string;
    };

interface Resolver {
  resolve: (value: string | null) => void;
}

class DialogStore {
  current = $state<DialogRequest | null>(null);
  private resolver: Resolver | null = null;

  message(options: {
    title: string;
    message: string;
    variant?: "info" | "warning" | "error";
    buttons?: DialogButton[];
  }): Promise<string> {
    return this.open({
      kind: "message",
      title: options.title,
      message: options.message,
      variant: options.variant ?? "info",
      buttons: options.buttons ?? [{ value: "ok", label: "OK", variant: "primary" }],
    }).then((value) => value ?? "ok");
  }

  confirm(options: {
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    danger?: boolean;
  }): Promise<boolean> {
    return this.message({
      title: options.title,
      message: options.message,
      variant: "warning",
      buttons: [
        {
          value: "cancel",
          label: options.cancelLabel ?? "Cancel",
          variant: "secondary",
        },
        {
          value: "ok",
          label: options.confirmLabel ?? "OK",
          variant: options.danger ? "danger" : "primary",
        },
      ],
    }).then((value) => value === "ok");
  }

  input(options: {
    title: string;
    message?: string;
    label: string;
    value?: string;
    placeholder?: string;
    submitLabel?: string;
  }): Promise<string | null> {
    return this.open({
      kind: "input",
      title: options.title,
      message: options.message ?? "",
      label: options.label,
      value: options.value ?? "",
      placeholder: options.placeholder ?? "",
      submitLabel: options.submitLabel ?? "Save",
    });
  }

  password(options: { title: string; message?: string; label: string }): Promise<string | null> {
    return this.open({
      kind: "password",
      title: options.title,
      message: options.message ?? "",
      label: options.label,
    });
  }

  close(value: string | null = null): void {
    const resolver = this.resolver;
    this.resolver = null;
    this.current = null;
    resolver?.resolve(value);
  }

  private open(request: DialogRequest): Promise<string | null> {
    // Resolve a previous dialog before replacing it so callers never hang.
    this.close(null);
    this.current = request;
    return new Promise((resolve) => {
      this.resolver = { resolve };
    });
  }
}

export const dialogs = new DialogStore();
