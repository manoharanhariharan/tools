export interface ContextItem {
  label: string;
  disabled?: boolean;
  action?: () => void;
  children?: ContextItem[];
}

export interface ContextCreator {
  label: string;
  placeholder: string;
  action: (value: string) => void;
}
