# Custom Confirm Dialog Usage

The app now uses a custom styled confirmation dialog that matches the app theme, replacing the native `window.confirm()`.

## Basic Usage

```tsx
import { useConfirm } from '../hooks/useConfirm';

function MyComponent() {
  const { confirm, ConfirmDialog } = useConfirm();

  const handleDelete = async () => {
    const confirmed = await confirm({
      title: 'Delete File',
      message: 'Are you sure you want to delete this file?',
      confirmText: 'Delete',
      cancelText: 'Cancel',
      variant: 'danger', // 'danger' | 'warning' | 'info'
    });

    if (confirmed) {
      // Proceed with deletion
    }
  };

  return (
    <>
      <ConfirmDialog />
      <button onClick={handleDelete}>Delete</button>
    </>
  );
}
```

## Options

- `title`: Dialog title (required)
- `message`: Dialog message/description (required)
- `confirmText`: Text for confirm button (default: 'Confirm')
- `cancelText`: Text for cancel button (default: 'Cancel')
- `variant`: Visual style - 'danger' (red), 'warning' (yellow), or 'info' (blue)

## Features

- ✅ Matches app theme (light/dark mode)
- ✅ Keyboard shortcuts (Enter to confirm, Escape to cancel)
- ✅ Click outside to cancel
- ✅ Promise-based API (works with async/await)
- ✅ Pure Tailwind CSS styling (no custom CSS)
- ✅ Three variants for different severity levels

## Variants

### Danger (Red)
Use for destructive actions like delete, discard, remove:
```tsx
variant: 'danger'
```

### Warning (Yellow)
Use for potentially risky actions:
```tsx
variant: 'warning'
```

### Info (Blue)
Use for informational confirmations:
```tsx
variant: 'info'
```
