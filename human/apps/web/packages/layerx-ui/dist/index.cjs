"use client";
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/index.ts
var index_exports = {};
__export(index_exports, {
  AmountText: () => AmountText,
  AppShell: () => AppShell,
  Avatar: () => Avatar,
  Badge: () => Badge,
  BalanceHeader: () => BalanceHeader,
  BankCard: () => BankCard,
  BellPopover: () => BellPopover,
  BottomTabBar: () => BottomTabBar,
  Button: () => Button,
  CalendarRangePicker: () => CalendarRangePicker,
  Card: () => Card,
  CardCarousel: () => CardCarousel,
  CodeEntry: () => CodeEntry,
  CodeInput: () => CodeInput,
  ConfirmDialog: () => ConfirmDialog,
  DetailDisclosure: () => DetailDisclosure,
  Divider: () => Divider,
  Drawer: () => Drawer,
  DrawerBody: () => DrawerBody,
  DrawerFooter: () => DrawerFooter,
  DrawerHeader: () => DrawerHeader,
  EmptyState: () => EmptyState,
  FilterBar: () => FilterBar,
  GlobalSearch: () => GlobalSearch,
  IconButton: () => IconButton,
  IconTile: () => IconTile,
  Input: () => Input,
  Keypad: () => Keypad,
  List: () => List,
  ListItem: () => ListItem,
  Modal: () => Modal,
  ModalBody: () => ModalBody,
  ModalFooter: () => ModalFooter,
  ModalHeader: () => ModalHeader,
  MoneyList: () => MoneyList,
  NotificationsArchive: () => NotificationsArchive,
  NotificationsScreen: () => NotificationsScreen,
  OptionList: () => OptionList,
  PlatformProvider: () => PlatformProvider,
  PlatformSwitch: () => PlatformSwitch,
  Popover: () => Popover,
  PopoverContent: () => PopoverContent,
  PopoverTrigger: () => PopoverTrigger,
  PrimaryAction: () => PrimaryAction,
  QuickActions: () => QuickActions,
  ResponsiveDialog: () => ResponsiveDialog,
  SearchInput: () => SearchInput,
  SectionHeader: () => SectionHeader,
  SegmentedControl: () => SegmentedControl,
  Sheet: () => Sheet,
  SheetBody: () => SheetBody,
  SheetDescription: () => SheetDescription,
  SheetFooter: () => SheetFooter,
  SheetHeader: () => SheetHeader,
  Sidebar: () => Sidebar,
  Skeleton: () => Skeleton,
  SkeletonRow: () => SkeletonRow,
  Spinner: () => Spinner,
  Stat: () => Stat,
  StatPair: () => StatPair,
  Switch: () => Switch,
  ViewAllChip: () => ViewAllChip,
  Wizard: () => Wizard,
  avatarVariants: () => avatarVariants,
  badgeVariants: () => badgeVariants,
  buttonVariants: () => buttonVariants,
  cardVariants: () => cardVariants,
  cn: () => cn,
  downloadCsv: () => downloadCsv,
  formatBalance: () => formatBalance,
  formatMoney: () => formatMoney,
  formatRecency: () => formatRecency,
  iconButtonVariants: () => iconButtonVariants,
  isFilterActive: () => isFilterActive,
  monthBandLabel: () => monthBandLabel,
  recencyOf: () => recencyOf,
  useMediaQuery: () => useMediaQuery,
  usePlatform: () => usePlatform
});
module.exports = __toCommonJS(index_exports);

// src/lib/utils.ts
var import_clsx = require("clsx");
var import_tailwind_merge = require("tailwind-merge");
function cn(...inputs) {
  return (0, import_tailwind_merge.twMerge)((0, import_clsx.clsx)(inputs));
}

// src/lib/format.ts
function formatMoney(value, opts = {}) {
  const { currency, signed = true, decimals = 2, locale = "en-US" } = opts;
  const symbol = opts.symbol ?? (currency === void 0 ? "$" : "");
  const abs = Math.abs(value);
  const num = abs.toLocaleString(locale, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals
  });
  const sign = value < 0 ? "- " : signed && value > 0 ? "+ " : "";
  const cur = currency ? ` ${currency}` : "";
  return `${sign}${symbol}${num}${cur}`;
}
function formatBalance(value, symbol = "$") {
  return `${symbol} ${value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
  })}`;
}
function formatRecency(date, now = /* @__PURE__ */ new Date()) {
  const mins = Math.round((now.getTime() - date.getTime()) / 6e4);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}
function monthBandLabel(date) {
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}
function downloadCsv(filename, header, rows) {
  const escape = (v) => {
    const s = String(v);
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const csv = [header, ...rows].map((r) => r.map(escape).join(",")).join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

// src/lib/platform.tsx
var React = __toESM(require("react"), 1);
var import_jsx_runtime = require("react/jsx-runtime");
var PlatformContext = React.createContext("auto");
var ResolvedContext = React.createContext("desktop");
function PlatformProvider({
  value = "auto",
  children
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime.jsx)(PlatformContext.Provider, { value, children });
}
function useMediaQuery(query) {
  const [matches, setMatches] = React.useState(false);
  React.useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);
  return matches;
}
function usePlatform(override) {
  const fromContext = React.useContext(PlatformContext);
  const setting = override ?? fromContext;
  const isMobileViewport = useMediaQuery("(max-width: 767px)");
  if (setting === "mobile") return "mobile";
  if (setting === "desktop") return "desktop";
  return isMobileViewport ? "mobile" : "desktop";
}
function PlatformSwitch({
  mobile,
  desktop,
  platform
}) {
  const resolved = usePlatform(platform);
  return /* @__PURE__ */ (0, import_jsx_runtime.jsx)(import_jsx_runtime.Fragment, { children: resolved === "mobile" ? mobile : desktop });
}

// src/lib/types.ts
function recencyOf(date, now = /* @__PURE__ */ new Date()) {
  const days = (now.getTime() - date.getTime()) / 864e5;
  if (days < 1) return "today";
  if (days < 7) return "week";
  return "month";
}

// src/components/button.tsx
var React2 = __toESM(require("react"), 1);
var import_react_slot = require("@radix-ui/react-slot");
var import_class_variance_authority = require("class-variance-authority");
var import_lucide_react = require("lucide-react");
var import_jsx_runtime2 = require("react/jsx-runtime");
var buttonVariants = (0, import_class_variance_authority.cva)(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap font-semibold transition-colors select-none outline-none focus-visible:ring-2 focus-visible:ring-accent/40 disabled:pointer-events-none disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        /** Signature solid black pill. */
        primary: "bg-primary text-primary-foreground hover:bg-primary-hover active:bg-primary-hover",
        /** White pill with a hairline border — pairs with primary in footers. */
        secondary: "bg-surface text-foreground border border-border-strong hover:bg-surface-sunken/60",
        /** Light gray filled pill. */
        soft: "bg-surface-sunken text-foreground hover:bg-border/60",
        /** Blue pill — for accent CTAs. */
        accent: "bg-accent text-accent-foreground hover:bg-accent-strong",
        /** Solid red pill for irreversible actions. */
        destructive: "bg-destructive text-destructive-foreground hover:opacity-90",
        /** Borderless. */
        ghost: "text-foreground hover:bg-surface-sunken",
        /** Text-only accent link. */
        link: "text-accent underline-offset-4 hover:underline h-auto px-0"
      },
      size: {
        sm: "h-9 px-4 text-sm rounded-full [&_svg]:size-4",
        md: "h-11 px-6 text-[15px] rounded-full [&_svg]:size-[18px]",
        lg: "h-[52px] px-7 text-base rounded-full [&_svg]:size-5",
        icon: "size-11 rounded-full [&_svg]:size-5"
      },
      fullWidth: {
        true: "w-full",
        false: ""
      }
    },
    defaultVariants: { variant: "primary", size: "md", fullWidth: false }
  }
);
var Button = React2.forwardRef(
  ({ className, variant, size, fullWidth, asChild = false, loading, children, disabled, ...props }, ref) => {
    const Comp = asChild ? import_react_slot.Slot : "button";
    return /* @__PURE__ */ (0, import_jsx_runtime2.jsxs)(
      Comp,
      {
        ref,
        disabled: disabled || loading,
        className: cn(buttonVariants({ variant, size, fullWidth, className })),
        ...props,
        children: [
          loading && /* @__PURE__ */ (0, import_jsx_runtime2.jsx)(import_lucide_react.Loader2, { className: "animate-spin", "aria-hidden": true }),
          children
        ]
      }
    );
  }
);
Button.displayName = "Button";
var iconButtonVariants = (0, import_class_variance_authority.cva)(
  "inline-flex items-center justify-center rounded-full transition-colors outline-none focus-visible:ring-2 focus-visible:ring-accent/40 disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-5",
  {
    variants: {
      variant: {
        /** White circle with hairline border — the design set's header buttons. */
        outline: "bg-surface border border-border text-foreground hover:bg-surface-sunken/60",
        soft: "bg-surface-sunken text-foreground hover:bg-border/60",
        ghost: "text-foreground hover:bg-surface-sunken",
        accent: "bg-accent text-accent-foreground hover:bg-accent-strong",
        primary: "bg-primary text-primary-foreground hover:bg-primary-hover"
      },
      size: {
        sm: "size-9 [&_svg]:size-4",
        md: "size-11",
        lg: "size-14 [&_svg]:size-6"
      }
    },
    defaultVariants: { variant: "outline", size: "md" }
  }
);
var IconButton = React2.forwardRef(
  ({ className, variant, size, ...props }, ref) => /* @__PURE__ */ (0, import_jsx_runtime2.jsx)("button", { ref, className: cn(iconButtonVariants({ variant, size, className })), ...props })
);
IconButton.displayName = "IconButton";

// src/components/input.tsx
var React3 = __toESM(require("react"), 1);
var import_lucide_react2 = require("lucide-react");
var import_jsx_runtime3 = require("react/jsx-runtime");
var Input = React3.forwardRef(
  ({ className, error, leading, trailing, ...props }, ref) => {
    return /* @__PURE__ */ (0, import_jsx_runtime3.jsxs)(
      "div",
      {
        className: cn(
          "flex h-12 items-center gap-2 rounded-md border bg-surface px-4 transition-colors",
          error ? "border-destructive focus-within:ring-2 focus-within:ring-destructive/25" : "border-border focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/20",
          props.disabled && "opacity-50",
          className
        ),
        children: [
          leading,
          /* @__PURE__ */ (0, import_jsx_runtime3.jsx)(
            "input",
            {
              ref,
              className: "h-full w-full min-w-0 bg-transparent text-[15px] text-foreground outline-none placeholder:text-faint-foreground",
              ...props
            }
          ),
          trailing
        ]
      }
    );
  }
);
Input.displayName = "Input";
var SearchInput = React3.forwardRef(
  ({ className, value, onClear, ...props }, ref) => {
    const hasValue = value !== void 0 ? String(value).length > 0 : false;
    return /* @__PURE__ */ (0, import_jsx_runtime3.jsxs)(
      "div",
      {
        className: cn(
          "flex h-11 items-center gap-2.5 rounded-full bg-surface border border-border px-4 transition-colors focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/20",
          className
        ),
        children: [
          /* @__PURE__ */ (0, import_jsx_runtime3.jsx)(import_lucide_react2.Search, { className: "size-[18px] shrink-0 text-muted-foreground", "aria-hidden": true }),
          /* @__PURE__ */ (0, import_jsx_runtime3.jsx)(
            "input",
            {
              ref,
              type: "search",
              value,
              className: "h-full w-full min-w-0 bg-transparent text-[15px] text-foreground outline-none placeholder:text-faint-foreground [&::-webkit-search-cancel-button]:hidden",
              ...props
            }
          ),
          hasValue && onClear && /* @__PURE__ */ (0, import_jsx_runtime3.jsx)(
            "button",
            {
              type: "button",
              onClick: onClear,
              "aria-label": "Clear search",
              className: "shrink-0 text-faint-foreground hover:text-muted-foreground",
              children: /* @__PURE__ */ (0, import_jsx_runtime3.jsx)(import_lucide_react2.X, { className: "size-4" })
            }
          )
        ]
      }
    );
  }
);
SearchInput.displayName = "SearchInput";

// src/components/badge.tsx
var import_class_variance_authority2 = require("class-variance-authority");
var import_jsx_runtime4 = require("react/jsx-runtime");
var badgeVariants = (0, import_class_variance_authority2.cva)(
  "inline-flex items-center gap-1 rounded-full font-semibold whitespace-nowrap [&_svg]:size-3",
  {
    variants: {
      variant: {
        /** Default gray pill — Active/Inactive, Settled/Pending. */
        neutral: "bg-surface-sunken text-foreground-secondary",
        success: "bg-success-soft text-success",
        destructive: "bg-destructive-soft text-destructive",
        warning: "bg-warning-soft text-warning",
        accent: "bg-accent-soft text-accent-strong",
        outline: "border border-border-strong text-foreground-secondary"
      },
      size: {
        sm: "h-6 px-2.5 text-xs",
        md: "h-7 px-3 text-[13px]"
      }
    },
    defaultVariants: { variant: "neutral", size: "md" }
  }
);
function Badge({ className, variant, size, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime4.jsx)("span", { className: cn(badgeVariants({ variant, size, className })), ...props });
}

// src/components/avatar.tsx
var React4 = __toESM(require("react"), 1);
var import_class_variance_authority3 = require("class-variance-authority");
var import_jsx_runtime5 = (
  // eslint-disable-next-line @next/next/no-img-element
  require("react/jsx-runtime")
);
var avatarVariants = (0, import_class_variance_authority3.cva)(
  "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full font-semibold select-none",
  {
    variants: {
      size: {
        xs: "size-7 text-[11px]",
        sm: "size-9 text-xs",
        md: "size-11 text-sm",
        lg: "size-14 text-base",
        xl: "size-20 text-xl"
      },
      tone: {
        /** Black tile with white initials — the design set's profile avatar. */
        primary: "bg-primary text-primary-foreground",
        accent: "bg-accent-soft text-accent-strong",
        neutral: "bg-surface-sunken text-foreground-secondary"
      }
    },
    defaultVariants: { size: "md", tone: "neutral" }
  }
);
function deriveInitials(name) {
  if (!name) return "";
  return name.split(" ").filter(Boolean).slice(0, 2).map((p) => p[0].toUpperCase()).join("");
}
var Avatar = React4.forwardRef(
  ({ className, size, tone, src, alt, initials, ...props }, ref) => {
    const [imgFailed, setImgFailed] = React4.useState(false);
    const showImage = src && !imgFailed;
    return /* @__PURE__ */ (0, import_jsx_runtime5.jsx)("span", { ref, className: cn(avatarVariants({ size, tone, className })), ...props, children: showImage ? /* @__PURE__ */ (0, import_jsx_runtime5.jsx)(
      "img",
      {
        src,
        alt: alt ?? "",
        className: "absolute inset-0 size-full object-cover",
        onError: () => setImgFailed(true)
      }
    ) : /* @__PURE__ */ (0, import_jsx_runtime5.jsx)("span", { "aria-hidden": true, children: initials ?? deriveInitials(alt) }) });
  }
);
Avatar.displayName = "Avatar";

// src/components/card.tsx
var React5 = __toESM(require("react"), 1);
var import_class_variance_authority4 = require("class-variance-authority");
var import_jsx_runtime6 = require("react/jsx-runtime");
var cardVariants = (0, import_class_variance_authority4.cva)("rounded-lg bg-surface", {
  variants: {
    elevation: {
      /** Hairline border + soft shadow — the design set's default card. */
      raised: "border border-border shadow-card",
      outline: "border border-border",
      flat: "bg-surface-sunken/60"
    },
    padding: {
      none: "",
      sm: "p-3",
      md: "p-4",
      lg: "p-5"
    }
  },
  defaultVariants: { elevation: "raised", padding: "md" }
});
var Card = React5.forwardRef(
  ({ className, elevation, padding, ...props }, ref) => /* @__PURE__ */ (0, import_jsx_runtime6.jsx)("div", { ref, className: cn(cardVariants({ elevation, padding, className })), ...props })
);
Card.displayName = "Card";

// src/components/switch.tsx
var React6 = __toESM(require("react"), 1);
var SwitchPrimitive = __toESM(require("@radix-ui/react-switch"), 1);
var import_jsx_runtime7 = require("react/jsx-runtime");
var Switch = React6.forwardRef(({ className, ...props }, ref) => /* @__PURE__ */ (0, import_jsx_runtime7.jsx)(
  SwitchPrimitive.Root,
  {
    ref,
    className: cn(
      "peer inline-flex h-[26px] w-[46px] shrink-0 cursor-pointer items-center rounded-full border border-transparent transition-colors outline-none",
      "bg-border-strong data-[state=checked]:bg-accent",
      "focus-visible:ring-2 focus-visible:ring-accent/30 disabled:cursor-not-allowed disabled:opacity-50",
      className
    ),
    ...props,
    children: /* @__PURE__ */ (0, import_jsx_runtime7.jsx)(
      SwitchPrimitive.Thumb,
      {
        className: cn(
          "pointer-events-none block size-[22px] rounded-full bg-white shadow-sm ring-0 transition-transform",
          "translate-x-[2px] data-[state=checked]:translate-x-[22px]"
        )
      }
    )
  }
));
Switch.displayName = "Switch";

// src/components/segmented-control.tsx
var import_jsx_runtime8 = require("react/jsx-runtime");
function SegmentedControl({
  options,
  value,
  onValueChange,
  className,
  size = "md",
  ...aria
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime8.jsx)(
    "div",
    {
      role: "tablist",
      className: cn(
        "flex w-full items-center rounded-full bg-surface-sunken p-1",
        size === "sm" ? "h-9" : "h-11",
        className
      ),
      ...aria,
      children: options.map((opt) => {
        const active = opt.value === value;
        return /* @__PURE__ */ (0, import_jsx_runtime8.jsx)(
          "button",
          {
            role: "tab",
            "aria-selected": active,
            type: "button",
            onClick: () => onValueChange(opt.value),
            className: cn(
              "flex h-full flex-1 items-center justify-center rounded-full font-semibold whitespace-nowrap transition-all outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
              size === "sm" ? "px-3 text-[13px]" : "px-4 text-sm",
              active ? "bg-surface text-foreground shadow-[0_1px_4px_rgb(0_0_0/0.10)]" : "text-faint-foreground hover:text-muted-foreground"
            ),
            children: opt.label
          },
          opt.value
        );
      })
    }
  );
}

// src/components/list.tsx
var import_lucide_react3 = require("lucide-react");
var import_jsx_runtime9 = require("react/jsx-runtime");
function List({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsx)(
    "div",
    {
      className: cn("flex flex-col divide-y divide-border/70", className),
      role: "list",
      ...props
    }
  );
}
function IconTile({
  className,
  tone = "neutral",
  shape = "square",
  ...props
}) {
  const tones = {
    neutral: "bg-surface-sunken text-foreground-secondary",
    accent: "bg-accent-soft text-accent",
    success: "bg-success-soft text-success",
    destructive: "bg-destructive-soft text-destructive"
  };
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsx)(
    "span",
    {
      className: cn(
        "inline-flex size-11 shrink-0 items-center justify-center [&_svg]:size-5",
        shape === "square" ? "rounded-md" : "rounded-full",
        tones[tone],
        className
      ),
      ...props
    }
  );
}
function ListItem({
  className,
  leading,
  title,
  subtitle,
  trailing,
  trailingCaption,
  navigates,
  onClick,
  ...props
}) {
  const interactive = Boolean(onClick) || navigates;
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsxs)(
    "div",
    {
      role: "listitem",
      tabIndex: interactive ? 0 : void 0,
      onClick,
      onKeyDown: onClick ? (e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick(e);
        }
      } : void 0,
      className: cn(
        "flex w-full items-center gap-3 py-3.5 text-left",
        interactive && "cursor-pointer transition-colors hover:bg-surface-sunken/40 -mx-2 px-2 rounded-md",
        className
      ),
      ...props,
      children: [
        leading,
        /* @__PURE__ */ (0, import_jsx_runtime9.jsxs)("span", { className: "flex min-w-0 flex-1 flex-col gap-0.5", children: [
          /* @__PURE__ */ (0, import_jsx_runtime9.jsx)("span", { className: "truncate text-[15px] font-semibold text-foreground", children: title }),
          subtitle && /* @__PURE__ */ (0, import_jsx_runtime9.jsx)("span", { className: "truncate text-[13px] text-muted-foreground", children: subtitle })
        ] }),
        (trailing || navigates) && /* @__PURE__ */ (0, import_jsx_runtime9.jsxs)("span", { className: "flex shrink-0 flex-col items-end gap-0.5", children: [
          /* @__PURE__ */ (0, import_jsx_runtime9.jsxs)("span", { className: "flex items-center gap-1.5", children: [
            trailing,
            navigates && /* @__PURE__ */ (0, import_jsx_runtime9.jsx)(import_lucide_react3.ChevronRight, { className: "size-4 text-faint-foreground", "aria-hidden": true })
          ] }),
          trailingCaption && /* @__PURE__ */ (0, import_jsx_runtime9.jsx)("span", { className: "text-xs text-faint-foreground", children: trailingCaption })
        ] })
      ]
    }
  );
}
function SectionHeader({
  title,
  action,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsxs)("div", { className: cn("flex items-center justify-between gap-3", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime9.jsx)("h3", { className: "text-[17px] font-bold text-foreground", children: title }),
    action
  ] });
}
function ViewAllChip({
  className,
  children = "View all",
  ...props
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsx)(
    "button",
    {
      type: "button",
      className: cn(
        "inline-flex h-8 items-center rounded-full bg-surface border border-border px-3.5 text-[13px] font-semibold text-foreground-secondary transition-colors hover:bg-surface-sunken/60",
        className
      ),
      ...props,
      children
    }
  );
}
function Divider({ className }) {
  return /* @__PURE__ */ (0, import_jsx_runtime9.jsx)("hr", { className: cn("border-0 border-t border-border", className) });
}

// src/components/amount.tsx
var import_jsx_runtime10 = require("react/jsx-runtime");
function AmountText({
  value,
  currency,
  locale,
  decimals,
  symbol,
  colorMode = "signed",
  className,
  ...props
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime10.jsx)(
    "span",
    {
      className: cn(
        "font-semibold tabular-nums",
        colorMode === "signed" && (value > 0 ? "text-success" : value < 0 ? "text-destructive" : "text-foreground"),
        colorMode === "neutral" && "text-foreground",
        className
      ),
      ...props,
      children: formatMoney(value, { currency, decimals, locale, symbol })
    }
  );
}

// src/components/stat.tsx
var import_jsx_runtime11 = require("react/jsx-runtime");
function Stat({
  value,
  label,
  className,
  align = "center"
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime11.jsxs)(
    "div",
    {
      className: cn(
        "flex flex-col gap-1",
        align === "center" ? "items-center text-center" : "items-start",
        className
      ),
      children: [
        /* @__PURE__ */ (0, import_jsx_runtime11.jsx)("span", { className: "text-2xl font-bold tabular-nums text-foreground", children: value }),
        /* @__PURE__ */ (0, import_jsx_runtime11.jsx)("span", { className: "text-sm text-muted-foreground", children: label })
      ]
    }
  );
}
function StatPair({
  left,
  right,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime11.jsxs)("div", { className: cn("grid grid-cols-2", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime11.jsx)(Stat, { value: left.value, label: left.label }),
    /* @__PURE__ */ (0, import_jsx_runtime11.jsx)(Stat, { value: right.value, label: right.label, className: "border-l border-border" })
  ] });
}

// src/components/empty-state.tsx
var import_jsx_runtime12 = require("react/jsx-runtime");
function EmptyState({
  icon,
  title,
  description,
  action,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime12.jsxs)(
    "div",
    {
      className: cn(
        "flex flex-col items-center gap-2.5 rounded-lg bg-surface px-6 py-10 text-center",
        className
      ),
      children: [
        icon && /* @__PURE__ */ (0, import_jsx_runtime12.jsx)("span", { className: "mb-1 inline-flex size-16 items-center justify-center rounded-full bg-surface-sunken text-muted-foreground [&_svg]:size-7", children: icon }),
        /* @__PURE__ */ (0, import_jsx_runtime12.jsx)("h3", { className: "text-[17px] font-bold text-foreground", children: title }),
        description && /* @__PURE__ */ (0, import_jsx_runtime12.jsx)("p", { className: "max-w-[280px] text-sm leading-relaxed text-muted-foreground", children: description }),
        action && /* @__PURE__ */ (0, import_jsx_runtime12.jsx)("div", { className: "mt-3", children: action })
      ]
    }
  );
}

// src/components/feedback.tsx
var import_lucide_react4 = require("lucide-react");
var import_jsx_runtime13 = require("react/jsx-runtime");
function Spinner({ className }) {
  return /* @__PURE__ */ (0, import_jsx_runtime13.jsx)(import_lucide_react4.Loader2, { className: cn("size-5 animate-spin text-muted-foreground", className), "aria-label": "Loading" });
}
function Skeleton({ className }) {
  return /* @__PURE__ */ (0, import_jsx_runtime13.jsx)("div", { className: cn("animate-pulse rounded-md bg-surface-sunken", className) });
}
function SkeletonRow() {
  return /* @__PURE__ */ (0, import_jsx_runtime13.jsxs)("div", { className: "flex items-center gap-3 py-3.5", children: [
    /* @__PURE__ */ (0, import_jsx_runtime13.jsx)(Skeleton, { className: "size-11 rounded-full" }),
    /* @__PURE__ */ (0, import_jsx_runtime13.jsxs)("div", { className: "flex flex-1 flex-col gap-2", children: [
      /* @__PURE__ */ (0, import_jsx_runtime13.jsx)(Skeleton, { className: "h-3.5 w-1/3" }),
      /* @__PURE__ */ (0, import_jsx_runtime13.jsx)(Skeleton, { className: "h-3 w-1/4" })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime13.jsx)(Skeleton, { className: "h-3.5 w-16" })
  ] });
}

// src/components/option-list.tsx
var RadioGroup = __toESM(require("@radix-ui/react-radio-group"), 1);
var import_jsx_runtime14 = require("react/jsx-runtime");
function OptionList({
  items,
  value,
  onValueChange,
  className,
  "aria-label": ariaLabel
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime14.jsx)(
    RadioGroup.Root,
    {
      value,
      onValueChange,
      className: cn("flex flex-col divide-y divide-border/70", className),
      "aria-label": ariaLabel,
      children: items.map((item) => {
        const checked = item.value === value;
        return /* @__PURE__ */ (0, import_jsx_runtime14.jsxs)(
          RadioGroup.Item,
          {
            value: item.value,
            className: "group flex w-full cursor-pointer items-center justify-between gap-3 py-4 text-left outline-none",
            children: [
              /* @__PURE__ */ (0, import_jsx_runtime14.jsxs)("span", { className: "flex min-w-0 flex-col gap-0.5", children: [
                /* @__PURE__ */ (0, import_jsx_runtime14.jsx)("span", { className: "text-[15px] font-medium text-foreground", children: item.label }),
                item.description && /* @__PURE__ */ (0, import_jsx_runtime14.jsx)("span", { className: "text-[13px] text-muted-foreground", children: item.description })
              ] }),
              /* @__PURE__ */ (0, import_jsx_runtime14.jsx)(
                "span",
                {
                  className: cn(
                    "inline-flex size-[22px] shrink-0 items-center justify-center rounded-full border-2 transition-colors",
                    checked ? "border-accent" : "border-border-strong group-hover:border-faint-foreground"
                  ),
                  "aria-hidden": true,
                  children: checked && /* @__PURE__ */ (0, import_jsx_runtime14.jsx)("span", { className: "size-3 rounded-full bg-accent" })
                }
              )
            ]
          },
          item.value
        );
      })
    }
  );
}

// src/components/balance-header.tsx
var React7 = __toESM(require("react"), 1);
var import_lucide_react5 = require("lucide-react");
var import_jsx_runtime15 = require("react/jsx-runtime");
function BalanceHeader({
  label,
  value,
  symbol = "$",
  change,
  hidden: hiddenProp,
  onHiddenChange,
  align = "left",
  className
}) {
  const [internalHidden, setInternalHidden] = React7.useState(false);
  const hidden = hiddenProp ?? internalHidden;
  const toggle = () => {
    const next = !hidden;
    setInternalHidden(next);
    onHiddenChange?.(next);
  };
  return /* @__PURE__ */ (0, import_jsx_runtime15.jsxs)("div", { className: cn("flex flex-col gap-1", align === "center" && "items-center", className), children: [
    label && /* @__PURE__ */ (0, import_jsx_runtime15.jsx)("span", { className: "text-sm text-muted-foreground", children: label }),
    /* @__PURE__ */ (0, import_jsx_runtime15.jsxs)("div", { className: "flex items-center gap-2.5", children: [
      /* @__PURE__ */ (0, import_jsx_runtime15.jsx)("span", { className: "text-[32px] leading-none font-extrabold tabular-nums tracking-tight text-foreground", children: hidden ? `${symbol} \u2022\u2022\u2022\u2022\u2022\u2022` : formatBalance(value, symbol) }),
      /* @__PURE__ */ (0, import_jsx_runtime15.jsx)(
        "button",
        {
          type: "button",
          onClick: toggle,
          "aria-label": hidden ? "Show balance" : "Hide balance",
          className: "inline-flex size-7 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-surface-sunken",
          children: hidden ? /* @__PURE__ */ (0, import_jsx_runtime15.jsx)(import_lucide_react5.EyeOff, { className: "size-[18px]" }) : /* @__PURE__ */ (0, import_jsx_runtime15.jsx)(import_lucide_react5.Eye, { className: "size-[18px]" })
        }
      )
    ] }),
    change && !hidden && /* @__PURE__ */ (0, import_jsx_runtime15.jsxs)("span", { className: "flex items-center gap-1.5 text-sm text-muted-foreground", children: [
      "1 day change:",
      /* @__PURE__ */ (0, import_jsx_runtime15.jsxs)(
        "span",
        {
          className: cn(
            "flex items-center gap-1 font-semibold",
            change.up ? "text-success" : "text-destructive"
          ),
          children: [
            change.text,
            change.up ? /* @__PURE__ */ (0, import_jsx_runtime15.jsx)(import_lucide_react5.TrendingUp, { className: "size-4", "aria-hidden": true }) : /* @__PURE__ */ (0, import_jsx_runtime15.jsx)(import_lucide_react5.TrendingDown, { className: "size-4", "aria-hidden": true })
          ]
        }
      )
    ] })
  ] });
}

// src/components/quick-actions.tsx
var import_jsx_runtime16 = require("react/jsx-runtime");
function QuickActions({
  actions,
  onAction,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime16.jsx)(
    "div",
    {
      className: cn("grid gap-2", className),
      style: { gridTemplateColumns: `repeat(${Math.min(actions.length, 5)}, minmax(0, 1fr))` },
      children: actions.map((a) => /* @__PURE__ */ (0, import_jsx_runtime16.jsxs)(
        "button",
        {
          type: "button",
          onClick: () => onAction?.(a.id),
          className: "group flex flex-col items-center gap-2 outline-none",
          children: [
            /* @__PURE__ */ (0, import_jsx_runtime16.jsx)("span", { className: "inline-flex size-12 items-center justify-center rounded-full border border-border bg-surface text-foreground transition-colors group-hover:bg-surface-sunken group-focus-visible:ring-2 group-focus-visible:ring-accent/30 [&_svg]:size-5", children: a.icon }),
            /* @__PURE__ */ (0, import_jsx_runtime16.jsx)("span", { className: "text-[13px] font-medium text-foreground-secondary", children: a.label })
          ]
        },
        a.id
      ))
    }
  );
}

// src/components/bank-card.tsx
var React8 = __toESM(require("react"), 1);
var import_jsx_runtime17 = require("react/jsx-runtime");
function BankCard({ data, className }) {
  const dark = data.theme === "dark";
  return /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)(
    "div",
    {
      className: cn(
        "relative flex aspect-[8/5] w-full flex-col justify-between overflow-hidden rounded-lg p-5 shadow-card",
        dark ? "bg-[#101418] text-white" : "bg-[linear-gradient(135deg,#eef3fa_0%,#e2eaf5_55%,#dbe5f2_100%)] text-foreground",
        className
      ),
      children: [
        /* @__PURE__ */ (0, import_jsx_runtime17.jsx)(
          "div",
          {
            "aria-hidden": true,
            className: cn(
              "pointer-events-none absolute inset-0",
              dark ? "bg-[radial-gradient(120%_90%_at_80%_0%,rgb(255_255_255/0.08),transparent_60%)]" : "bg-[radial-gradient(120%_90%_at_80%_0%,rgb(255_255_255/0.7),transparent_60%)]"
            )
          }
        ),
        /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: "relative flex items-start justify-between gap-3", children: [
          /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: "text-[13px] font-bold tracking-[0.12em] uppercase", children: data.holder }),
          data.status && /* @__PURE__ */ (0, import_jsx_runtime17.jsx)(Badge, { variant: data.status.tone ?? "success", size: "sm", className: "bg-surface/80", children: data.status.label })
        ] }),
        /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: "relative flex items-end justify-between gap-4", children: [
          /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: "flex flex-col gap-1", children: [
            /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: cn("text-xs", dark ? "text-white/60" : "text-muted-foreground"), children: data.kind ?? "Virtual card" }),
            /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: "text-[15px] font-semibold tracking-[0.06em] tabular-nums", children: data.number })
          ] }),
          /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: "flex flex-col items-end gap-1", children: [
            /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: cn("text-xs", dark ? "text-white/60" : "text-muted-foreground"), children: data.balance ? data.balanceLabel ?? "Balance" : "Expiry" }),
            /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: "text-[15px] font-semibold tabular-nums", children: data.balance ?? data.expiry })
          ] })
        ] }),
        /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: "relative flex items-center justify-between", children: [
          /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: cn("text-lg font-black tracking-tight", dark ? "text-white" : "text-foreground"), children: "\u224B" }),
          /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("span", { className: "text-sm font-black tracking-[0.08em] uppercase italic", children: data.brand ?? "VISA" })
        ] })
      ]
    }
  );
}
function CardCarousel({
  cards,
  renderCard,
  className
}) {
  const [active, setActive] = React8.useState(0);
  const trackRef = React8.useRef(null);
  const onScroll = () => {
    const el = trackRef.current;
    if (!el) return;
    const i = Math.round(el.scrollLeft / el.clientWidth);
    setActive(Math.max(0, Math.min(cards.length - 1, i)));
  };
  return /* @__PURE__ */ (0, import_jsx_runtime17.jsxs)("div", { className: cn("flex flex-col gap-3", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime17.jsx)(
      "div",
      {
        ref: trackRef,
        onScroll,
        className: "lx-scroll flex snap-x snap-mandatory gap-3 overflow-x-auto",
        children: cards.map((c, i) => /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("div", { className: "w-full shrink-0 snap-center", children: renderCard ? renderCard(c, i) : /* @__PURE__ */ (0, import_jsx_runtime17.jsx)(BankCard, { data: c }) }, i))
      }
    ),
    cards.length > 1 && /* @__PURE__ */ (0, import_jsx_runtime17.jsx)("div", { className: "flex items-center justify-center gap-1.5", "aria-hidden": true, children: cards.map((_, i) => /* @__PURE__ */ (0, import_jsx_runtime17.jsx)(
      "span",
      {
        className: cn(
          "size-1.5 rounded-full transition-colors",
          i === active ? "bg-foreground" : "bg-border-strong"
        )
      },
      i
    )) })
  ] });
}

// src/components/sheet.tsx
var React9 = __toESM(require("react"), 1);
var Dialog = __toESM(require("@radix-ui/react-dialog"), 1);
var import_jsx_runtime18 = require("react/jsx-runtime");
function Sheet({ open, onOpenChange, children, portalContainer }) {
  const dragStartY = React9.useRef(null);
  const startDrag = (event) => {
    if (!(event.target instanceof Element) || event.target.closest("[data-sheet-drag-handle]") === null) {
      return;
    }
    dragStartY.current = event.clientY;
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const finishDrag = (event) => {
    const startY = dragStartY.current;
    dragStartY.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (startY !== null && event.clientY - startY >= 72) {
      onOpenChange(false);
    }
  };
  return /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(Dialog.Root, { open, onOpenChange, children: /* @__PURE__ */ (0, import_jsx_runtime18.jsxs)(Dialog.Portal, { container: portalContainer ?? void 0, children: [
    /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(Dialog.Overlay, { className: "fixed inset-0 z-40 bg-black/40 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out" }),
    /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(
      Dialog.Content,
      {
        onPointerDown: startDrag,
        onPointerUp: finishDrag,
        onPointerCancel: () => {
          dragStartY.current = null;
        },
        className: cn(
          "fixed inset-x-0 bottom-0 z-50 mx-auto flex max-h-[calc(100dvh-env(safe-area-inset-top))] w-full max-w-lg flex-col overscroll-contain",
          "rounded-t-sheet bg-surface shadow-overlay outline-none",
          "data-[state=open]:animate-sheet-up data-[state=closed]:animate-sheet-down"
        ),
        children
      }
    )
  ] }) });
}
function SheetHeader({
  title,
  className,
  children
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime18.jsxs)("div", { className: cn("flex flex-col items-stretch", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime18.jsx)("div", { className: "flex justify-center pt-2.5 pb-1", "aria-hidden": true, children: /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(
      "span",
      {
        "data-sheet-drag-handle": true,
        className: "h-5 w-12 touch-none rounded-full before:mx-auto before:mt-2 before:block before:h-1 before:w-10 before:rounded-full before:bg-border-strong"
      }
    ) }),
    (title || children) && /* @__PURE__ */ (0, import_jsx_runtime18.jsxs)("div", { className: "border-b border-border px-5 pt-2 pb-4", children: [
      title && /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(Dialog.Title, { className: "text-lg font-bold text-foreground", children: title }),
      children
    ] })
  ] });
}
function SheetDescription({
  className,
  ...props
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(Dialog.Description, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(
    "p",
    {
      className: cn("text-[15px] leading-relaxed text-foreground-secondary", className),
      ...props
    }
  ) });
}
function SheetBody({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(
    "div",
    {
      className: cn(
        "lx-scroll flex-1 overflow-y-auto px-5 pt-4 pb-[max(1rem,env(safe-area-inset-bottom))]",
        className
      ),
      ...props
    }
  );
}
function SheetFooter({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime18.jsx)(
    "div",
    {
      className: cn(
        "grid auto-cols-fr grid-flow-col gap-3 border-t border-border/0 px-5 pt-2 pb-[max(1.5rem,env(safe-area-inset-bottom))]",
        className
      ),
      ...props
    }
  );
}

// src/components/modal.tsx
var Dialog2 = __toESM(require("@radix-ui/react-dialog"), 1);
var import_lucide_react6 = require("lucide-react");
var import_jsx_runtime19 = require("react/jsx-runtime");
function Modal({ open, onOpenChange, children, portalContainer, className }) {
  return /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(Dialog2.Root, { open, onOpenChange, children: /* @__PURE__ */ (0, import_jsx_runtime19.jsxs)(Dialog2.Portal, { container: portalContainer ?? void 0, children: [
    /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(Dialog2.Overlay, { className: "fixed inset-0 z-40 bg-black/40 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out" }),
    /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(
      Dialog2.Content,
      {
        className: cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-2rem-env(safe-area-inset-top)-env(safe-area-inset-bottom))] w-[calc(100vw-2rem)] max-w-[440px] -translate-x-1/2 -translate-y-1/2 flex-col overscroll-contain",
          "rounded-xl bg-surface p-6 shadow-overlay outline-none",
          "data-[state=open]:animate-modal-in data-[state=closed]:animate-modal-out",
          className
        ),
        children
      }
    )
  ] }) });
}
function ModalHeader({
  title,
  description,
  onClose,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime19.jsxs)("div", { className: cn("flex items-start justify-between gap-4", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime19.jsxs)("div", { className: "flex flex-col gap-1.5", children: [
      /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(Dialog2.Title, { className: "text-lg font-bold text-foreground", children: title }),
      description && /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(Dialog2.Description, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime19.jsx)("p", { className: "text-sm leading-relaxed text-muted-foreground", children: description }) })
    ] }),
    onClose && /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(
      "button",
      {
        type: "button",
        onClick: onClose,
        "aria-label": "Close",
        className: "-mt-1 -mr-1 inline-flex size-11 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-surface-sunken",
        children: /* @__PURE__ */ (0, import_jsx_runtime19.jsx)(import_lucide_react6.X, { className: "size-4" })
      }
    )
  ] });
}
function ModalBody({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime19.jsx)("div", { className: cn("lx-scroll mt-4 flex-1 overflow-y-auto", className), ...props });
}
function ModalFooter({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime19.jsx)("div", { className: cn("mt-6 flex items-center justify-end gap-3", className), ...props });
}

// src/components/drawer.tsx
var Dialog3 = __toESM(require("@radix-ui/react-dialog"), 1);
var import_lucide_react7 = require("lucide-react");
var import_jsx_runtime20 = require("react/jsx-runtime");
function Drawer({ open, onOpenChange, children, portalContainer, width = 420 }) {
  return /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(Dialog3.Root, { open, onOpenChange, children: /* @__PURE__ */ (0, import_jsx_runtime20.jsxs)(Dialog3.Portal, { container: portalContainer ?? void 0, children: [
    /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(Dialog3.Overlay, { className: "fixed inset-0 z-40 bg-black/30 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out" }),
    /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(
      Dialog3.Content,
      {
        style: { width: `min(${typeof width === "number" ? `${width}px` : width}, 100vw)` },
        className: cn(
          "fixed top-0 right-0 z-50 flex h-dvh flex-col overscroll-contain bg-surface pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] shadow-overlay outline-none",
          "data-[state=open]:animate-drawer-in data-[state=closed]:animate-drawer-out"
        ),
        children
      }
    )
  ] }) });
}
function DrawerHeader({
  title,
  description,
  onClose,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime20.jsxs)("div", { className: cn("flex items-start justify-between gap-4 border-b border-border p-5", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime20.jsxs)("div", { className: "flex flex-col gap-1", children: [
      /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(Dialog3.Title, { className: "text-lg font-bold text-foreground", children: title }),
      description && /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(Dialog3.Description, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime20.jsx)("p", { className: "text-sm text-muted-foreground", children: description }) })
    ] }),
    onClose && /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(
      "button",
      {
        type: "button",
        onClick: onClose,
        "aria-label": "Close",
        className: "inline-flex size-11 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-surface-sunken",
        children: /* @__PURE__ */ (0, import_jsx_runtime20.jsx)(import_lucide_react7.X, { className: "size-4" })
      }
    )
  ] });
}
function DrawerBody({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime20.jsx)("div", { className: cn("lx-scroll flex-1 overflow-y-auto p-5", className), ...props });
}
function DrawerFooter({ className, ...props }) {
  return /* @__PURE__ */ (0, import_jsx_runtime20.jsx)("div", { className: cn("flex items-center justify-end gap-3 border-t border-border p-4", className), ...props });
}

// src/components/responsive-dialog.tsx
var Dialog4 = __toESM(require("@radix-ui/react-dialog"), 1);
var import_react_visually_hidden = require("@radix-ui/react-visually-hidden");
var import_jsx_runtime21 = require("react/jsx-runtime");
function ResponsiveDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  platform,
  portalContainer
}) {
  const resolved = usePlatform(platform);
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(Sheet, { open, onOpenChange, portalContainer, children: [
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(SheetHeader, { title }),
      /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(SheetBody, { className: "flex flex-col gap-4", children: [
        description && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(SheetDescription, { children: description }),
        children
      ] }),
      footer && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(SheetFooter, { children: footer })
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(Modal, { open, onOpenChange, portalContainer, children: [
    /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(ModalHeader, { title, description }),
    children && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(ModalBody, { children }),
    footer && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(ModalFooter, { children: footer })
  ] });
}
function ConfirmDialog({
  open,
  onOpenChange,
  icon,
  title,
  consequence,
  confirm,
  cancel,
  platform,
  portalContainer
}) {
  const resolved = usePlatform(platform);
  const footer = /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(import_jsx_runtime21.Fragment, { children: [
    cancel && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(
      Button,
      {
        variant: cancel.variant ?? "secondary",
        size: "lg",
        fullWidth: resolved === "mobile",
        loading: cancel.loading,
        onClick: cancel.onClick ?? (() => onOpenChange(false)),
        children: cancel.label
      }
    ),
    /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(
      Button,
      {
        variant: confirm.variant ?? "primary",
        size: "lg",
        fullWidth: resolved === "mobile",
        loading: confirm.loading,
        onClick: confirm.onClick ?? (() => onOpenChange(false)),
        children: confirm.label
      }
    )
  ] });
  const body = /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)("div", { className: cn("flex flex-col items-center gap-3 text-center", resolved === "desktop" && "py-2"), children: [
    icon && /* @__PURE__ */ (0, import_jsx_runtime21.jsx)("span", { className: "inline-flex size-16 items-center justify-center rounded-full bg-surface-sunken text-foreground-secondary [&_svg]:size-7", children: icon }),
    /* @__PURE__ */ (0, import_jsx_runtime21.jsx)("p", { className: "text-[15px] leading-relaxed text-foreground-secondary", children: consequence })
  ] });
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(Sheet, { open, onOpenChange, portalContainer, children: [
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(import_react_visually_hidden.VisuallyHidden, { children: /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(Dialog4.Title, { children: title }) }),
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(SheetHeader, {}),
      /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(SheetBody, { className: "flex flex-col gap-4 pt-2", children: [
        /* @__PURE__ */ (0, import_jsx_runtime21.jsx)("h2", { className: "text-center text-xl font-bold text-foreground", "aria-hidden": true, children: title }),
        body
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(SheetFooter, { children: footer })
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)(Modal, { open, onOpenChange, portalContainer, children: [
    /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(import_react_visually_hidden.VisuallyHidden, { children: /* @__PURE__ */ (0, import_jsx_runtime21.jsx)(Dialog4.Title, { children: title }) }),
    /* @__PURE__ */ (0, import_jsx_runtime21.jsxs)("div", { className: "flex flex-col gap-4", children: [
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)("h2", { className: "text-center text-xl font-bold text-foreground", "aria-hidden": true, children: title }),
      body,
      /* @__PURE__ */ (0, import_jsx_runtime21.jsx)("div", { className: "mt-2 grid auto-cols-fr grid-flow-col gap-3", children: footer })
    ] })
  ] });
}

// src/components/popover.tsx
var React10 = __toESM(require("react"), 1);
var PopoverPrimitive = __toESM(require("@radix-ui/react-popover"), 1);
var import_jsx_runtime22 = require("react/jsx-runtime");
var Popover = PopoverPrimitive.Root;
var PopoverTrigger = PopoverPrimitive.Trigger;
var PopoverContent = React10.forwardRef(({ className, align = "start", sideOffset = 8, ...props }, ref) => /* @__PURE__ */ (0, import_jsx_runtime22.jsx)(PopoverPrimitive.Portal, { children: /* @__PURE__ */ (0, import_jsx_runtime22.jsx)(
  PopoverPrimitive.Content,
  {
    ref,
    align,
    sideOffset,
    className: cn(
      "z-50 w-auto min-w-[220px] rounded-lg border border-border bg-surface p-2 shadow-overlay outline-none",
      "data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out",
      className
    ),
    ...props
  }
) }));
PopoverContent.displayName = "PopoverContent";

// src/components/calendar-range-picker.tsx
var import_react_day_picker = require("react-day-picker");
var import_date_fns = require("date-fns");
var import_lucide_react8 = require("lucide-react");
var import_jsx_runtime23 = require("react/jsx-runtime");
var dayPickerClassNames = {
  root: "text-sm text-foreground",
  months: "flex gap-4",
  month_caption: "flex items-center justify-between px-1 pb-2 font-semibold",
  caption_label: "text-sm font-semibold",
  nav: "flex items-center gap-1",
  button_previous: "inline-flex size-7 items-center justify-center rounded-full hover:bg-surface-sunken",
  button_next: "inline-flex size-7 items-center justify-center rounded-full hover:bg-surface-sunken",
  weekdays: "flex",
  weekday: "w-9 flex-1 text-center text-xs font-medium text-faint-foreground py-1",
  week: "flex",
  day: "w-9 flex-1 p-0",
  day_button: "size-9 w-full rounded-full text-sm transition-colors outline-none hover:bg-surface-sunken",
  outside: "text-faint-foreground/60",
  disabled: "opacity-40"
};
var dayPickerModifiersClassNames = {
  today: "font-bold text-accent-strong",
  selected: "bg-accent text-accent-foreground hover:bg-accent",
  range_start: "rounded-full",
  range_end: "rounded-full",
  range_middle: "bg-accent-soft! text-foreground! rounded-none!"
};
function CalendarRangePicker({
  value,
  onChange,
  placeholder = "Select range",
  className
}) {
  const label = value?.from && value?.to ? `${(0, import_date_fns.format)(value.from, "MMM d, yyyy")} \u2013 ${(0, import_date_fns.format)(value.to, "MMM d, yyyy")}` : value?.from ? `${(0, import_date_fns.format)(value.from, "MMM d, yyyy")} \u2013 \u2026` : placeholder;
  return /* @__PURE__ */ (0, import_jsx_runtime23.jsxs)(Popover, { children: [
    /* @__PURE__ */ (0, import_jsx_runtime23.jsx)(PopoverTrigger, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime23.jsxs)(
      "button",
      {
        type: "button",
        className: cn(
          "flex h-11 items-center justify-between gap-2 rounded-md border border-border bg-surface px-3.5 text-sm font-medium transition-colors",
          "hover:bg-surface-sunken/50 focus-visible:ring-2 focus-visible:ring-accent/30 outline-none",
          value?.from ? "text-foreground" : "text-faint-foreground",
          className
        ),
        children: [
          /* @__PURE__ */ (0, import_jsx_runtime23.jsxs)("span", { className: "flex items-center gap-2", children: [
            /* @__PURE__ */ (0, import_jsx_runtime23.jsx)(import_lucide_react8.CalendarDays, { className: "size-4 text-muted-foreground", "aria-hidden": true }),
            label
          ] }),
          /* @__PURE__ */ (0, import_jsx_runtime23.jsx)(import_lucide_react8.ChevronDown, { className: "size-4 text-faint-foreground", "aria-hidden": true })
        ]
      }
    ) }),
    /* @__PURE__ */ (0, import_jsx_runtime23.jsx)(PopoverContent, { className: "p-3", align: "end", children: /* @__PURE__ */ (0, import_jsx_runtime23.jsx)(
      import_react_day_picker.DayPicker,
      {
        mode: "range",
        selected: value,
        onSelect: onChange,
        numberOfMonths: 1,
        classNames: dayPickerClassNames,
        modifiersClassNames: dayPickerModifiersClassNames
      }
    ) })
  ] });
}

// src/components/code-input.tsx
var React11 = __toESM(require("react"), 1);
var import_lucide_react9 = require("lucide-react");
var import_jsx_runtime24 = require("react/jsx-runtime");
function CodeInput({
  length = 6,
  value,
  onChange,
  onComplete,
  error,
  disabled,
  autoFocus,
  readOnly,
  className,
  ...aria
}) {
  const inputRef = React11.useRef(null);
  const [focused, setFocused] = React11.useState(false);
  React11.useEffect(() => {
    if (autoFocus && !readOnly && !disabled) {
      inputRef.current?.focus({ preventScroll: true });
    }
  }, []);
  const commit = (next) => {
    const clean = next.replace(/\D/g, "").slice(0, length);
    onChange(clean);
    if (clean.length === length) onComplete?.(clean);
  };
  const activeIndex = Math.min(value.length, length - 1);
  return /* @__PURE__ */ (0, import_jsx_runtime24.jsxs)(
    "div",
    {
      className: cn("relative", className),
      onClick: () => !readOnly && inputRef.current?.focus(),
      children: [
        /* @__PURE__ */ (0, import_jsx_runtime24.jsx)(
          "input",
          {
            ref: inputRef,
            type: "text",
            inputMode: "numeric",
            autoComplete: "one-time-code",
            "aria-invalid": error || void 0,
            "aria-label": aria["aria-label"] ?? "Verification code",
            className: cn(
              "absolute inset-0 h-full w-full opacity-0",
              readOnly ? "pointer-events-none" : "cursor-text"
            ),
            value,
            disabled,
            readOnly,
            onFocus: () => setFocused(true),
            onBlur: () => setFocused(false),
            onChange: (e) => commit(e.target.value),
            onPaste: (e) => {
              e.preventDefault();
              commit(e.clipboardData.getData("text"));
            }
          }
        ),
        /* @__PURE__ */ (0, import_jsx_runtime24.jsx)("div", { className: "flex items-center justify-center gap-2.5", "aria-hidden": true, children: Array.from({ length }).map((_, i) => {
          const char = value[i];
          const isActive = focused && !disabled && i === activeIndex;
          return /* @__PURE__ */ (0, import_jsx_runtime24.jsx)(
            "span",
            {
              className: cn(
                "flex size-12 items-center justify-center rounded-md border bg-surface text-xl font-semibold tabular-nums transition-colors",
                error ? "border-destructive text-destructive" : isActive ? "border-accent ring-2 ring-accent/20 text-foreground" : "border-border text-foreground",
                disabled && "opacity-50"
              ),
              children: char ?? ""
            },
            i
          );
        }) })
      ]
    }
  );
}
function Keypad({
  onDigit,
  onBackspace,
  className
}) {
  const keys = [
    { main: "1" },
    { main: "2", sub: "ABC" },
    { main: "3", sub: "DEF" },
    { main: "4", sub: "GHI" },
    { main: "5", sub: "JKL" },
    { main: "6", sub: "MNO" },
    { main: "7", sub: "PQRS" },
    { main: "8", sub: "TUV" },
    { main: "9", sub: "WXYZ" },
    { main: "+*#" },
    { main: "0" },
    { main: "back" }
  ];
  return /* @__PURE__ */ (0, import_jsx_runtime24.jsx)("div", { className: cn("grid grid-cols-3 gap-px overflow-hidden rounded-lg bg-border", className), children: keys.map(
    (k) => k.main === "back" ? /* @__PURE__ */ (0, import_jsx_runtime24.jsx)(
      "button",
      {
        type: "button",
        "aria-label": "Backspace",
        onClick: onBackspace,
        className: "flex h-14 items-center justify-center bg-surface text-foreground transition-colors active:bg-surface-sunken",
        children: /* @__PURE__ */ (0, import_jsx_runtime24.jsx)(import_lucide_react9.Delete, { className: "size-5" })
      },
      "back"
    ) : /* @__PURE__ */ (0, import_jsx_runtime24.jsxs)(
      "button",
      {
        type: "button",
        onClick: () => /^\d$/.test(k.main) && onDigit(k.main),
        className: "flex h-14 flex-col items-center justify-center gap-0 bg-surface text-foreground transition-colors active:bg-surface-sunken",
        children: [
          /* @__PURE__ */ (0, import_jsx_runtime24.jsx)("span", { className: "text-xl font-semibold leading-tight", children: k.main }),
          k.sub && /* @__PURE__ */ (0, import_jsx_runtime24.jsx)("span", { className: "text-[9px] font-semibold tracking-[0.18em] text-muted-foreground", children: k.sub })
        ]
      },
      k.main
    )
  ) });
}

// src/components/app-shell.tsx
var import_lucide_react10 = require("lucide-react");
var import_jsx_runtime25 = require("react/jsx-runtime");
var DEFAULT_ICONS = {
  home: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Home, { className: "size-[22px]" }),
  agents: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Bot, { className: "size-[22px]" }),
  activity: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Activity, { className: "size-[22px]" }),
  more: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Grid2x2, { className: "size-[22px]" })
};
function BottomTabBar({
  items,
  activeId,
  onNavigate,
  onFab,
  fabIcon,
  className
}) {
  const left = items.slice(0, 2);
  const right = items.slice(2, 4);
  const Tab = ({ item }) => {
    const active = item.id === activeId;
    return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
      "button",
      {
        type: "button",
        onClick: () => onNavigate?.(item.id),
        "aria-current": active ? "page" : void 0,
        className: "relative flex flex-col items-center gap-0.5 py-1 outline-none",
        children: [
          /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: cn("transition-colors", active ? "text-accent" : "text-faint-foreground"), children: item.icon ?? DEFAULT_ICONS[item.id] ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Grid2x2, { className: "size-[22px]" }) }),
          /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(
            "span",
            {
              className: cn(
                "text-[11px] font-semibold transition-colors",
                active ? "text-accent" : "text-faint-foreground"
              ),
              children: item.label
            }
          ),
          !!item.badge && /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "absolute -top-1 right-1/2 translate-x-4 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-bold text-destructive-foreground", children: item.badge })
        ]
      }
    );
  };
  return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
    "nav",
    {
      "aria-label": "Primary",
      className: cn(
        "relative z-30 grid grid-cols-5 items-end border-t border-border bg-surface px-2 pt-1.5 pb-[max(0.5rem,env(safe-area-inset-bottom))]",
        className
      ),
      children: [
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Tab, { item: left[0] }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Tab, { item: left[1] }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("div", { className: "flex justify-center", children: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(
          "button",
          {
            type: "button",
            onClick: onFab,
            "aria-label": "New action",
            className: "-mt-7 inline-flex size-14 items-center justify-center rounded-full bg-accent text-accent-foreground shadow-overlay transition-transform active:scale-95",
            children: fabIcon ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Plus, { className: "size-6" })
          }
        ) }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Tab, { item: right[0] }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Tab, { item: right[1] })
      ]
    }
  );
}
function Sidebar({
  items,
  activeId,
  onNavigate,
  logo,
  explorerLabel = "Explorer",
  onExplorer,
  settingsLabel = "Settings",
  onSettings,
  user,
  className
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
    "aside",
    {
      className: cn(
        "flex h-full w-[248px] shrink-0 flex-col border-r border-border bg-surface",
        className
      ),
      children: [
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("div", { className: "flex h-16 items-center px-5 text-lg font-extrabold tracking-tight text-foreground", children: logo ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("span", { className: "flex items-center gap-2", children: [
          /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "inline-flex size-7 items-center justify-center rounded-md bg-primary text-[13px] font-black text-primary-foreground", children: "LX" }),
          "LayerX"
        ] }) }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("nav", { "aria-label": "Primary", className: "flex flex-1 flex-col gap-1 px-3 py-2", children: [
          items.map((item) => {
            const active = item.id === activeId;
            return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
              "button",
              {
                type: "button",
                onClick: () => onNavigate?.(item.id),
                "aria-current": active ? "page" : void 0,
                className: cn(
                  "flex items-center gap-3 rounded-md px-3 py-2.5 text-[15px] font-semibold transition-colors outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
                  active ? "bg-surface-sunken text-foreground" : "text-muted-foreground hover:bg-surface-sunken/60 hover:text-foreground"
                ),
                children: [
                  /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: cn("[&_svg]:size-5", active ? "text-accent" : "text-faint-foreground"), children: item.icon ?? DEFAULT_ICONS[item.id] ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Grid2x2, { className: "size-5" }) }),
                  /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "flex-1 text-left", children: item.label }),
                  !!item.badge && /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-destructive px-1.5 text-xs font-bold text-destructive-foreground", children: item.badge })
                ]
              },
              item.id
            );
          }),
          /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
            "button",
            {
              type: "button",
              onClick: onExplorer,
              className: "mt-4 flex items-center gap-3 rounded-md px-3 py-2.5 text-[15px] font-semibold text-muted-foreground transition-colors hover:bg-surface-sunken/60 hover:text-foreground",
              children: [
                /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Compass, { className: "size-5 text-faint-foreground" }),
                /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "flex-1 text-left", children: explorerLabel })
              ]
            }
          )
        ] }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "border-t border-border p-3", children: [
          /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
            "button",
            {
              type: "button",
              onClick: onSettings,
              className: "flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-[15px] font-semibold text-muted-foreground transition-colors hover:bg-surface-sunken/60 hover:text-foreground",
              children: [
                /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Settings, { className: "size-5 text-faint-foreground" }),
                /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "flex-1 text-left", children: settingsLabel })
              ]
            }
          ),
          user && /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "mt-1 flex items-center gap-3 rounded-md px-3 py-2", children: [
            /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Avatar, { alt: user.name, src: user.avatarSrc, size: "sm", tone: "primary" }),
            /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "flex min-w-0 flex-col", children: [
              /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "truncate text-sm font-semibold text-foreground", children: user.name }),
              user.subtitle && /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "truncate text-xs text-muted-foreground", children: user.subtitle })
            ] })
          ] })
        ] })
      ]
    }
  );
}
function AppShell({
  nav,
  activeNav,
  onNavigate,
  onPrimaryAction,
  primaryActionLabel = "New",
  primaryActionIcon,
  user,
  onSearch,
  onNotifications,
  notificationCount,
  notificationControl,
  onExplorer,
  onSettings,
  logo,
  title,
  headerActions,
  platform,
  className,
  children
}) {
  const resolved = usePlatform(platform);
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: cn("flex h-dvh flex-col bg-background", className), children: [
      /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("header", { className: "flex items-center gap-3 border-b border-border bg-surface px-4 pt-[max(0.75rem,env(safe-area-inset-top))] pb-3", children: [
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(Avatar, { alt: user?.name ?? "Account", src: user?.avatarSrc, initials: user?.initials, size: "sm", tone: "primary" }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
          "button",
          {
            type: "button",
            onClick: onSearch,
            className: "flex h-10 flex-1 items-center gap-2.5 rounded-full border border-border bg-surface px-4 text-[15px] text-faint-foreground transition-colors hover:bg-surface-sunken/50",
            children: [
              /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Search, { className: "size-4", "aria-hidden": true }),
              "Search"
            ]
          }
        ),
        notificationControl ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "relative", children: [
          /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(IconButton, { variant: "outline", size: "sm", onClick: onNotifications, "aria-label": "Notifications", children: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Bell, {}) }),
          !!notificationCount && /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "absolute -top-0.5 -right-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-bold text-destructive-foreground", children: notificationCount })
        ] })
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("main", { className: "lx-scroll flex-1 overflow-y-auto", children }),
      /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(
        BottomTabBar,
        {
          items: nav,
          activeId: activeNav,
          onNavigate,
          onFab: onPrimaryAction,
          fabIcon: primaryActionIcon
        }
      )
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: cn("flex h-dvh bg-background", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(
      Sidebar,
      {
        items: nav,
        activeId: activeNav,
        onNavigate,
        logo,
        onExplorer,
        onSettings,
        user: user ? { name: user.name, subtitle: "LayerX account", avatarSrc: user.avatarSrc } : void 0
      }
    ),
    /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "flex min-w-0 flex-1 flex-col", children: [
      /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("header", { className: "flex h-16 shrink-0 items-center gap-4 border-b border-border bg-surface px-6", children: [
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("h1", { className: "text-lg font-bold text-foreground", children: title }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("div", { className: "flex-1" }),
        /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)(
          "button",
          {
            type: "button",
            onClick: onSearch,
            className: "flex h-10 w-64 items-center gap-2.5 rounded-full border border-border bg-surface px-4 text-sm text-faint-foreground transition-colors hover:bg-surface-sunken/50",
            children: [
              /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Search, { className: "size-4", "aria-hidden": true }),
              /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "flex-1 text-left", children: "Search" }),
              /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("kbd", { className: "rounded border border-border bg-surface-sunken px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground", children: "\u2318K" })
            ]
          }
        ),
        notificationControl ?? /* @__PURE__ */ (0, import_jsx_runtime25.jsxs)("div", { className: "relative", children: [
          /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(IconButton, { variant: "outline", size: "sm", onClick: onNotifications, "aria-label": "Notifications", children: /* @__PURE__ */ (0, import_jsx_runtime25.jsx)(import_lucide_react10.Bell, {}) }),
          !!notificationCount && /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("span", { className: "absolute -top-0.5 -right-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-bold text-destructive-foreground", children: notificationCount })
        ] }),
        headerActions
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime25.jsx)("main", { className: "lx-scroll flex-1 overflow-y-auto", children })
    ] })
  ] });
}

// src/components/primary-action.tsx
var import_jsx_runtime26 = require("react/jsx-runtime");
function PrimaryAction({
  children,
  platform,
  position = "footer",
  className,
  ...props
}) {
  const resolved = usePlatform(platform);
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime26.jsx)(
      "div",
      {
        className: cn(
          "sticky bottom-0 z-20 -mx-4 mt-auto bg-[linear-gradient(to_top,var(--background)_60%,transparent)] px-4 pt-6 pb-[max(1rem,env(safe-area-inset-bottom))]"
        ),
        children: /* @__PURE__ */ (0, import_jsx_runtime26.jsx)(Button, { size: "lg", fullWidth: true, className, ...props, children })
      }
    );
  }
  return /* @__PURE__ */ (0, import_jsx_runtime26.jsx)(
    Button,
    {
      size: position === "header" ? "md" : "lg",
      className: cn(position === "footer" && "min-w-[180px]", className),
      ...props,
      children
    }
  );
}

// src/components/detail.tsx
var React12 = __toESM(require("react"), 1);
var import_lucide_react11 = require("lucide-react");
var import_jsx_runtime27 = require("react/jsx-runtime");
function DetailDisclosure({
  open,
  onOpenChange,
  title,
  children,
  mobileVariant = "sheet",
  desktopVariant = "drawer",
  platform,
  portalContainer,
  summary
}) {
  const resolved = usePlatform(platform);
  const disclosureId = React12.useId();
  if (resolved === "desktop" && desktopVariant === "inline") {
    return /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)("div", { className: "overflow-hidden rounded-lg border border-border bg-surface", children: [
      /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)(
        "button",
        {
          type: "button",
          "aria-expanded": open,
          "aria-controls": disclosureId,
          onClick: () => onOpenChange(!open),
          className: "flex w-full items-center justify-between gap-3 px-5 py-4 text-left font-semibold text-foreground transition-colors hover:bg-surface-sunken/40",
          children: [
            /* @__PURE__ */ (0, import_jsx_runtime27.jsx)("span", { children: summary ?? title }),
            /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(
              import_lucide_react11.ChevronDown,
              {
                className: cn("size-4 text-muted-foreground transition-transform", open && "rotate-180")
              }
            )
          ]
        }
      ),
      /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(
        "div",
        {
          id: disclosureId,
          role: "region",
          className: cn(
            "grid transition-[grid-template-rows] duration-300",
            open ? "grid-rows-[1fr]" : "grid-rows-[0fr]"
          ),
          children: /* @__PURE__ */ (0, import_jsx_runtime27.jsx)("div", { className: "overflow-hidden", children: /* @__PURE__ */ (0, import_jsx_runtime27.jsx)("div", { className: "border-t border-border px-5 py-4", children }) })
        }
      )
    ] });
  }
  if (resolved === "mobile" && mobileVariant === "pushed") {
    if (!open) return null;
    return /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)("div", { className: "fixed inset-0 z-50 flex flex-col bg-background pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] animate-fade-in", children: [
      /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)("header", { className: "flex items-center gap-3 border-b border-border bg-surface px-4 py-3", children: [
        /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(IconButton, { variant: "outline", size: "sm", onClick: () => onOpenChange(false), "aria-label": "Back", children: /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(import_lucide_react11.ArrowLeft, {}) }),
        /* @__PURE__ */ (0, import_jsx_runtime27.jsx)("h2", { className: "text-[17px] font-bold text-foreground", children: title })
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime27.jsx)("div", { className: "lx-scroll flex-1 overflow-y-auto p-4", children })
    ] });
  }
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)(Sheet, { open, onOpenChange, portalContainer, children: [
      /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(SheetHeader, { title }),
      /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(SheetBody, { children })
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime27.jsxs)(Drawer, { open, onOpenChange, portalContainer, children: [
    /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(DrawerHeader, { title, onClose: () => onOpenChange(false) }),
    /* @__PURE__ */ (0, import_jsx_runtime27.jsx)(DrawerBody, { children })
  ] });
}

// src/components/filters.tsx
var React13 = __toESM(require("react"), 1);
var import_lucide_react12 = require("lucide-react");
var import_react_day_picker2 = require("react-day-picker");
var import_jsx_runtime28 = require("react/jsx-runtime");
function isFilterActive(v) {
  if (!v) return false;
  if (typeof v === "string") return v.length > 0 && v !== "all";
  return Boolean(v.from);
}
function filterSummary(def, v) {
  if (!isFilterActive(v)) return null;
  if (def.type === "options" && typeof v === "string") {
    return def.options?.find((o) => o.value === v)?.label ?? null;
  }
  return "Custom range";
}
function FilterBar({
  filters,
  values,
  onChange,
  platform,
  portalContainer,
  className
}) {
  const resolved = usePlatform(platform);
  const [sheetOpen, setSheetOpen] = React13.useState(false);
  const [draft, setDraft] = React13.useState(values);
  const appliedCount = filters.filter((f) => isFilterActive(values[f.id])).length;
  const openSheet = () => {
    setDraft(values);
    setSheetOpen(true);
  };
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(import_jsx_runtime28.Fragment, { children: [
      /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(
        "button",
        {
          type: "button",
          onClick: openSheet,
          className: cn(
            "flex h-11 items-center gap-2 rounded-full border border-border bg-surface px-4 text-sm font-semibold text-foreground-secondary transition-colors hover:bg-surface-sunken/60",
            className
          ),
          children: [
            /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(import_lucide_react12.ListFilter, { className: "size-4", "aria-hidden": true }),
            "Filter",
            appliedCount > 0 && /* @__PURE__ */ (0, import_jsx_runtime28.jsx)("span", { className: "inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-accent px-1.5 text-xs font-bold text-accent-foreground", children: appliedCount })
          ]
        }
      ),
      /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(Sheet, { open: sheetOpen, onOpenChange: setSheetOpen, portalContainer, children: [
        /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(SheetHeader, { title: "Filter" }),
        /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(SheetBody, { className: "flex flex-col gap-6", children: filters.map((def) => /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)("section", { className: "flex flex-col gap-1", children: [
          /* @__PURE__ */ (0, import_jsx_runtime28.jsx)("h4", { className: "pb-1 text-[15px] font-bold text-foreground", children: def.label }),
          def.type === "options" ? /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
            OptionList,
            {
              "aria-label": def.label,
              items: def.options ?? [],
              value: draft[def.id] ?? def.options?.[0]?.value ?? "",
              onValueChange: (v) => setDraft((d) => ({ ...d, [def.id]: v }))
            }
          ) : /* @__PURE__ */ (0, import_jsx_runtime28.jsx)("div", { className: "rounded-md border border-border p-2", children: /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
            import_react_day_picker2.DayPicker,
            {
              mode: "range",
              numberOfMonths: 1,
              selected: draft[def.id] ?? void 0,
              onSelect: (r) => setDraft((d) => ({ ...d, [def.id]: r })),
              classNames: { ...dayPickerClassNames, root: "w-full text-sm text-foreground" },
              modifiersClassNames: dayPickerModifiersClassNames
            }
          ) })
        ] }, def.id)) }),
        /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(SheetFooter, { children: [
          /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
            Button,
            {
              variant: "secondary",
              size: "lg",
              onClick: () => {
                const cleared = {};
                filters.forEach((f) => cleared[f.id] = f.type === "options" ? "all" : void 0);
                setDraft(cleared);
                onChange(cleared);
                setSheetOpen(false);
              },
              children: "Clear"
            }
          ),
          /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
            Button,
            {
              size: "lg",
              onClick: () => {
                onChange(draft);
                setSheetOpen(false);
              },
              children: "Apply"
            }
          )
        ] })
      ] })
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime28.jsx)("div", { className: cn("flex flex-wrap items-center gap-2", className), children: filters.map((def) => {
    const v = values[def.id];
    const summary = filterSummary(def, v);
    const active = isFilterActive(v);
    if (def.type === "date-range") {
      return /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
        CalendarRangePicker,
        {
          value: v ?? void 0,
          onChange: (r) => onChange({ ...values, [def.id]: r }),
          placeholder: def.label
        },
        def.id
      );
    }
    return /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(Popover, { children: [
      /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(PopoverTrigger, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime28.jsxs)(
        "button",
        {
          type: "button",
          className: cn(
            "flex h-10 items-center gap-2 rounded-full border px-4 text-sm font-semibold transition-colors outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
            active ? "border-accent/40 bg-accent-soft text-accent-strong" : "border-border bg-surface text-foreground-secondary hover:bg-surface-sunken/60"
          ),
          children: [
            summary ?? def.label,
            /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(import_lucide_react12.ChevronDown, { className: "size-4 opacity-60", "aria-hidden": true })
          ]
        }
      ) }),
      /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(PopoverContent, { className: "w-[240px] p-2", children: /* @__PURE__ */ (0, import_jsx_runtime28.jsx)(
        OptionList,
        {
          "aria-label": def.label,
          items: def.options ?? [],
          value: v ?? "all",
          onValueChange: (nv) => onChange({ ...values, [def.id]: nv })
        }
      ) })
    ] }, def.id);
  }) });
}

// src/components/money-list.tsx
var React14 = __toESM(require("react"), 1);
var import_lucide_react13 = require("lucide-react");
var import_lucide_react14 = require("lucide-react");
var import_jsx_runtime29 = require("react/jsx-runtime");
function defaultLeading(item) {
  const incoming = item.amount >= 0;
  return /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(
    IconTile,
    {
      shape: "circle",
      tone: incoming ? "accent" : "neutral",
      className: "size-10 [&_svg]:size-4",
      children: incoming ? /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(import_lucide_react14.ArrowDownLeft, {}) : /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(import_lucide_react14.ArrowUpRight, {})
    }
  );
}
function statusTone(status) {
  if (!status) return "neutral";
  const s = status.toLowerCase();
  if (s === "settled" || s === "completed" || s === "active") return "success";
  if (s === "failed" || s === "blocked") return "destructive";
  return "neutral";
}
function MoneyBands({
  groups,
  onItemClick
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("div", { className: "flex flex-col gap-5", children: groups.map((g) => /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("section", { children: [
    /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "flex items-baseline justify-between pb-1", children: [
      /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("h4", { className: "text-sm font-bold text-muted-foreground", children: g.label }),
      /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "text-sm font-semibold tabular-nums text-foreground-secondary", children: formatMoney(g.subtotal, { currency: g.currency, signed: false }) })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(List, { children: g.items.map((item) => /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(
      ListItem,
      {
        leading: item.leading ?? defaultLeading(item),
        title: item.title,
        subtitle: item.status ? /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("span", { className: "flex items-center gap-1.5", children: [
          item.subtitle,
          /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(Badge, { variant: statusTone(item.status), size: "sm", children: item.status })
        ] }) : item.subtitle,
        trailing: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(AmountText, { value: item.amount, currency: item.currency }),
        trailingCaption: item.date.toLocaleDateString("en-US", {
          day: "numeric",
          month: "short",
          year: "numeric"
        }),
        onClick: onItemClick ? () => onItemClick(item) : void 0
      },
      item.id
    )) })
  ] }, g.id)) });
}
function MoneyTable({
  groups,
  onItemClick,
  exportName = "transactions.csv",
  maxHeight = 560
}) {
  const [sortKey, setSortKey] = React14.useState("date");
  const [sortDir, setSortDir] = React14.useState("desc");
  const toggleSort = (key) => {
    if (key === sortKey) setSortDir((d) => d === "asc" ? "desc" : "asc");
    else {
      setSortKey(key);
      setSortDir("desc");
    }
  };
  const sortedGroups = React14.useMemo(() => {
    const cmp = (a, b) => {
      let v = 0;
      if (sortKey === "date") v = a.date.getTime() - b.date.getTime();
      if (sortKey === "title") v = a.title.localeCompare(b.title);
      if (sortKey === "amount") v = a.amount - b.amount;
      return sortDir === "asc" ? v : -v;
    };
    return groups.map((g) => ({ ...g, items: [...g.items].sort(cmp) }));
  }, [groups, sortKey, sortDir]);
  const doExport = () => {
    const rows = groups.flatMap(
      (g) => g.items.map((i) => [
        g.label,
        i.title,
        i.subtitle ?? "",
        i.status ?? "",
        i.date.toISOString().slice(0, 10),
        i.amount.toFixed(2),
        i.currency ?? ""
      ])
    );
    downloadCsv(exportName, ["Group", "Title", "Subtitle", "Status", "Date", "Amount", "Currency"], rows);
  };
  const SortButton = ({ id, children }) => /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)(
    "button",
    {
      type: "button",
      onClick: () => toggleSort(id),
      className: cn(
        "inline-flex items-center gap-1 text-xs font-bold tracking-wide uppercase transition-colors",
        sortKey === id ? "text-foreground" : "text-faint-foreground hover:text-muted-foreground"
      ),
      children: [
        children,
        sortKey === id && (sortDir === "asc" ? /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(import_lucide_react13.ArrowUpNarrowWide, { className: "size-3.5" }) : /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(import_lucide_react13.ArrowDownWideNarrow, { className: "size-3.5" }))
      ]
    }
  );
  return /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "overflow-hidden rounded-lg border border-border bg-surface", children: [
    /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "flex items-center justify-between border-b border-border px-4 py-2.5", children: [
      /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("span", { className: "text-sm font-semibold text-foreground-secondary", children: [
        groups.reduce((n, g) => n + g.items.length, 0),
        " records"
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)(Button, { variant: "soft", size: "sm", onClick: doExport, children: [
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(import_lucide_react13.Download, {}),
        "Export"
      ] })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("div", { className: "lx-scroll overflow-y-auto", style: { maxHeight }, children: /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("table", { className: "w-full border-collapse text-sm", children: [
      /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("thead", { className: "sticky top-0 z-20 bg-surface", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("tr", { className: "border-b border-border text-left", children: [
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("th", { className: "px-4 py-2.5 font-medium", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(SortButton, { id: "title", children: "Description" }) }),
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("th", { className: "px-4 py-2.5 font-medium", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(SortButton, { id: "date", children: "Date" }) }),
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("th", { className: "px-4 py-2.5 font-medium", children: "Status" }),
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("th", { className: "px-4 py-2.5 text-right font-medium", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "inline-flex justify-end", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(SortButton, { id: "amount", children: "Amount" }) }) })
      ] }) }),
      sortedGroups.map((g) => /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("tbody", { children: [
        /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("tr", { children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(
          "td",
          {
            colSpan: 4,
            className: "sticky top-[41px] z-10 border-b border-border bg-surface-sunken/70 px-4 py-1.5 backdrop-blur",
            children: /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "flex items-baseline justify-between", children: [
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "text-xs font-bold text-muted-foreground", children: g.label }),
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "text-xs font-semibold tabular-nums text-foreground-secondary", children: formatMoney(g.subtotal, { currency: g.currency, signed: false }) })
            ] })
          }
        ) }),
        g.items.map((item) => /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)(
          "tr",
          {
            onClick: onItemClick ? () => onItemClick(item) : void 0,
            className: cn(
              "border-b border-border/60 transition-colors last:border-0 hover:bg-surface-sunken/40",
              onItemClick && "cursor-pointer"
            ),
            children: [
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("td", { className: "px-4 py-3", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "flex items-center gap-3", children: [
                item.leading ?? defaultLeading(item),
                /* @__PURE__ */ (0, import_jsx_runtime29.jsxs)("div", { className: "flex min-w-0 flex-col", children: [
                  /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "truncate font-semibold text-foreground", children: item.title }),
                  item.subtitle && /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("span", { className: "truncate text-xs text-muted-foreground", children: item.subtitle })
                ] })
              ] }) }),
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("td", { className: "px-4 py-3 whitespace-nowrap text-muted-foreground tabular-nums", children: item.date.toLocaleDateString("en-US", {
                day: "numeric",
                month: "short",
                year: "numeric"
              }) }),
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("td", { className: "px-4 py-3", children: item.status && /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(Badge, { variant: statusTone(item.status), size: "sm", children: item.status }) }),
              /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("td", { className: "px-4 py-3 text-right", children: /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(AmountText, { value: item.amount, currency: item.currency }) })
            ]
          },
          item.id
        ))
      ] }, g.id))
    ] }) })
  ] });
}
function MoneyList({
  groups,
  onItemClick,
  platform,
  exportName,
  className
}) {
  const resolved = usePlatform(platform);
  return /* @__PURE__ */ (0, import_jsx_runtime29.jsx)("div", { className, children: resolved === "mobile" ? /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(MoneyBands, { groups, onItemClick }) : /* @__PURE__ */ (0, import_jsx_runtime29.jsx)(MoneyTable, { groups, onItemClick, exportName }) });
}

// src/components/wizard.tsx
var React15 = __toESM(require("react"), 1);
var import_lucide_react15 = require("lucide-react");
var import_jsx_runtime30 = require("react/jsx-runtime");
function Wizard({
  steps,
  summary,
  onComplete,
  onCancel,
  completeLabel = "Confirm",
  summaryTitle = "What will happen",
  platform,
  className
}) {
  const resolved = usePlatform(platform);
  const [index, setIndex] = React15.useState(0);
  const step = steps[index];
  const isLast = index === steps.length - 1;
  const canContinue = step.canContinue ? step.canContinue() : true;
  const next = () => {
    if (isLast) onComplete?.();
    else setIndex((i) => Math.min(i + 1, steps.length - 1));
  };
  const back = () => {
    if (index === 0) onCancel?.();
    else setIndex((i) => Math.max(0, i - 1));
  };
  const stepBody = /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "flex flex-col gap-2", children: [
    /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("h2", { className: "text-xl font-bold text-foreground", children: step.title }),
    step.description && /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("p", { className: "text-[15px] leading-relaxed text-muted-foreground", children: step.description }),
    /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("div", { className: "pt-4", children: step.render() })
  ] });
  if (resolved === "mobile") {
    return /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: cn("flex h-full min-h-0 flex-1 flex-col", className), children: [
      /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "flex items-center gap-3 px-4 pt-2 pb-4", children: [
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(IconButton, { variant: "outline", size: "sm", onClick: back, "aria-label": "Back", children: /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(import_lucide_react15.ArrowLeft, {}) }),
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("div", { className: "flex flex-1 items-center gap-1.5", "aria-hidden": true, children: steps.map((s, i) => /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(
          "span",
          {
            className: cn(
              "h-1 flex-1 rounded-full transition-colors",
              i <= index ? "bg-foreground" : "bg-border"
            )
          },
          s.id
        )) }),
        /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("span", { className: "text-xs font-semibold text-muted-foreground tabular-nums", children: [
          index + 1,
          "/",
          steps.length
        ] })
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "lx-scroll flex min-h-0 flex-1 flex-col overflow-y-auto px-4", children: [
        stepBody,
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(PrimaryAction, { onClick: next, disabled: !canContinue, platform: "mobile", children: isLast ? completeLabel : "Continue" })
      ] })
    ] });
  }
  return /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: cn("grid min-h-0 flex-1 grid-cols-[1fr_340px] gap-8", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "flex min-h-0 flex-col", children: [
      /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("ol", { className: "flex items-center gap-2 pb-6", "aria-label": "Progress", children: steps.map((s, i) => /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("li", { className: "flex items-center gap-2", children: [
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(
          "span",
          {
            className: cn(
              "inline-flex size-6 items-center justify-center rounded-full text-xs font-bold",
              i < index ? "bg-success text-success-foreground" : i === index ? "bg-primary text-primary-foreground" : "bg-surface-sunken text-faint-foreground"
            ),
            children: i < index ? /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(import_lucide_react15.Check, { className: "size-3.5" }) : i + 1
          }
        ),
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(
          "span",
          {
            className: cn(
              "text-sm font-semibold",
              i === index ? "text-foreground" : "text-muted-foreground"
            ),
            children: s.label
          }
        ),
        i < steps.length - 1 && /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("span", { className: "mx-1 h-px w-6 bg-border", "aria-hidden": true })
      ] }, s.id)) }),
      /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("div", { className: "lx-scroll min-h-0 flex-1 overflow-y-auto pr-2", children: stepBody }),
      /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "flex items-center gap-3 border-t border-border pt-4", children: [
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(Button, { variant: "secondary", onClick: back, children: index === 0 ? "Cancel" : "Back" }),
        /* @__PURE__ */ (0, import_jsx_runtime30.jsx)(Button, { onClick: next, disabled: !canContinue, className: "min-w-[160px]", children: isLast ? completeLabel : "Continue" })
      ] })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("aside", { className: "sticky top-0 h-fit rounded-lg border border-border bg-surface p-5 shadow-card", children: [
      /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("h3", { className: "text-sm font-bold tracking-wide text-muted-foreground uppercase", children: summaryTitle }),
      /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("dl", { className: "mt-3 flex flex-col divide-y divide-border/70", children: [
        (summary ?? []).map((item) => /* @__PURE__ */ (0, import_jsx_runtime30.jsxs)("div", { className: "flex items-center justify-between gap-4 py-3", children: [
          /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("dt", { className: "text-sm text-muted-foreground", children: item.label }),
          /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("dd", { className: "text-right text-sm font-semibold text-foreground", children: item.value })
        ] }, item.label)),
        (!summary || summary.length === 0) && /* @__PURE__ */ (0, import_jsx_runtime30.jsx)("p", { className: "py-3 text-sm text-faint-foreground", children: "Your choices will appear here as you go." })
      ] })
    ] })
  ] });
}

// src/components/search.tsx
var React16 = __toESM(require("react"), 1);
var Dialog5 = __toESM(require("@radix-ui/react-dialog"), 1);
var import_cmdk = require("cmdk");
var import_lucide_react16 = require("lucide-react");
var import_jsx_runtime31 = require("react/jsx-runtime");
function CommandBar({
  open,
  onOpenChange,
  groups,
  onSelect,
  placeholder = "Search agents, transactions, actions\u2026",
  portalContainer
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(Dialog5.Root, { open, onOpenChange, children: /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(Dialog5.Portal, { container: portalContainer ?? void 0, children: [
    /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(Dialog5.Overlay, { className: "fixed inset-0 z-40 bg-black/40 data-[state=open]:animate-fade-in" }),
    /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(
      Dialog5.Content,
      {
        className: cn(
          "fixed top-[18%] left-1/2 z-50 w-[calc(100vw-2rem)] max-w-[560px] -translate-x-1/2",
          "overflow-hidden rounded-xl bg-surface shadow-overlay outline-none",
          "data-[state=open]:animate-fade-in"
        ),
        children: [
          /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(Dialog5.Title, { className: "sr-only", children: "Search" }),
          /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(import_cmdk.Command, { label: "Global search", className: "flex flex-col", children: [
            /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("div", { className: "flex items-center gap-3 border-b border-border px-4", children: [
              /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(import_lucide_react16.Search, { className: "size-[18px] shrink-0 text-muted-foreground", "aria-hidden": true }),
              /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(
                import_cmdk.Command.Input,
                {
                  autoFocus: true,
                  placeholder,
                  className: "h-14 w-full bg-transparent text-[15px] text-foreground outline-none placeholder:text-faint-foreground"
                }
              ),
              /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("kbd", { className: "shrink-0 rounded border border-border bg-surface-sunken px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground", children: "ESC" })
            ] }),
            /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(import_cmdk.Command.List, { className: "lx-scroll max-h-[320px] overflow-y-auto p-2", children: [
              /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(import_cmdk.Command.Empty, { className: "py-10 text-center text-sm text-muted-foreground", children: "No results found." }),
              groups.map((g) => /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(
                import_cmdk.Command.Group,
                {
                  heading: g.label,
                  className: "[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-bold [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-faint-foreground [&_[cmdk-group-heading]]:uppercase",
                  children: g.items.map((item) => /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(
                    import_cmdk.Command.Item,
                    {
                      value: `${item.title} ${item.subtitle ?? ""} ${(item.keywords ?? []).join(" ")}`,
                      onSelect: () => {
                        onSelect?.(item);
                        onOpenChange(false);
                      },
                      className: "flex cursor-pointer items-center gap-3 rounded-md px-3 py-2.5 data-[selected=true]:bg-surface-sunken",
                      children: [
                        item.icon && /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-sunken text-foreground-secondary [&_svg]:size-4", children: item.icon }),
                        /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("span", { className: "flex min-w-0 flex-col", children: [
                          /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "truncate text-sm font-semibold text-foreground", children: item.title }),
                          item.subtitle && /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "truncate text-xs text-muted-foreground", children: item.subtitle })
                        ] })
                      ]
                    },
                    item.id
                  ))
                },
                g.id
              ))
            ] })
          ] })
        ]
      }
    )
  ] }) });
}
function SearchScreen({
  open,
  onOpenChange,
  groups,
  onSelect,
  recents,
  placeholder = "Search"
}) {
  const [query, setQuery] = React16.useState("");
  React16.useEffect(() => {
    if (open) setQuery("");
  }, [open]);
  if (!open) return null;
  const q = query.trim().toLowerCase();
  const matches = (item) => !q || item.title.toLowerCase().includes(q) || item.subtitle?.toLowerCase().includes(q) || item.keywords?.some((k) => k.toLowerCase().includes(q));
  const shownGroups = groups.map((g) => ({ ...g, items: g.items.filter(matches) })).filter((g) => g.items.length > 0);
  return /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("div", { className: "fixed inset-0 z-50 flex flex-col bg-background animate-fade-in", children: [
    /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("header", { className: "flex items-center gap-3 border-b border-border bg-surface px-4 py-3", children: [
      /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(IconButton, { variant: "outline", size: "sm", onClick: () => onOpenChange(false), "aria-label": "Back", children: /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(import_lucide_react16.ArrowLeft, {}) }),
      /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("div", { className: "flex h-10 flex-1 items-center gap-2.5 rounded-full border border-border bg-surface px-4 focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/20", children: [
        /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(import_lucide_react16.Search, { className: "size-4 shrink-0 text-muted-foreground", "aria-hidden": true }),
        /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(
          "input",
          {
            autoFocus: true,
            value: query,
            onChange: (e) => setQuery(e.target.value),
            placeholder,
            className: "w-full bg-transparent text-[15px] text-foreground outline-none placeholder:text-faint-foreground"
          }
        )
      ] })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("div", { className: "lx-scroll flex-1 overflow-y-auto p-4", children: [
      !q && recents && recents.length > 0 && /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("section", { children: [
        /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("h4", { className: "pb-1 text-xs font-bold tracking-wide text-faint-foreground uppercase", children: "Recent" }),
        recents.map((item) => /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(
          "button",
          {
            type: "button",
            onClick: () => {
              onSelect?.(item);
              onOpenChange(false);
            },
            className: "flex w-full items-center gap-3 rounded-md py-2.5 text-left",
            children: [
              /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-sunken text-muted-foreground [&_svg]:size-4", children: item.icon ?? /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(import_lucide_react16.Clock, {}) }),
              /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("span", { className: "min-w-0 flex-1", children: [
                /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "block truncate text-[15px] font-semibold text-foreground", children: item.title }),
                item.subtitle && /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "block truncate text-[13px] text-muted-foreground", children: item.subtitle })
              ] })
            ]
          },
          item.id
        ))
      ] }),
      shownGroups.map((g) => /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("section", { className: "pt-3", children: [
        /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("h4", { className: "pb-1 text-xs font-bold tracking-wide text-faint-foreground uppercase", children: g.label }),
        g.items.map((item) => /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)(
          "button",
          {
            type: "button",
            onClick: () => {
              onSelect?.(item);
              onOpenChange(false);
            },
            className: "flex w-full items-center gap-3 rounded-md py-2.5 text-left",
            children: [
              item.icon && /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-sunken text-foreground-secondary [&_svg]:size-4", children: item.icon }),
              /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("span", { className: "min-w-0 flex-1", children: [
                /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "block truncate text-[15px] font-semibold text-foreground", children: item.title }),
                item.subtitle && /* @__PURE__ */ (0, import_jsx_runtime31.jsx)("span", { className: "block truncate text-[13px] text-muted-foreground", children: item.subtitle })
              ] })
            ]
          },
          item.id
        ))
      ] }, g.id)),
      q && shownGroups.length === 0 && /* @__PURE__ */ (0, import_jsx_runtime31.jsxs)("p", { className: "py-10 text-center text-sm text-muted-foreground", children: [
        "No results for \u201C",
        query,
        "\u201D."
      ] })
    ] })
  ] });
}
function GlobalSearch({
  open,
  onOpenChange,
  groups,
  onSelect,
  recents,
  placeholder,
  enableHotkey = true,
  platform,
  portalContainer
}) {
  const resolved = usePlatform(platform);
  React16.useEffect(() => {
    if (!enableHotkey || resolved !== "desktop") return;
    const onKey = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enableHotkey, resolved, open, onOpenChange]);
  return resolved === "mobile" ? /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(
    SearchScreen,
    {
      open,
      onOpenChange,
      groups,
      onSelect,
      recents,
      placeholder
    }
  ) : /* @__PURE__ */ (0, import_jsx_runtime31.jsx)(
    CommandBar,
    {
      open,
      onOpenChange,
      groups,
      onSelect,
      placeholder,
      portalContainer
    }
  );
}

// src/components/code-entry.tsx
var import_jsx_runtime32 = require("react/jsx-runtime");
function CodeEntry({
  length = 6,
  value,
  onChange,
  onComplete,
  error,
  errorText,
  resendIn,
  onResend,
  platform,
  className
}) {
  const resolved = usePlatform(platform);
  const mobile = resolved === "mobile";
  return /* @__PURE__ */ (0, import_jsx_runtime32.jsxs)("div", { className: cn("flex flex-col gap-4", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime32.jsx)(
      CodeInput,
      {
        length,
        value,
        onChange,
        onComplete,
        error,
        readOnly: mobile,
        autoFocus: !mobile,
        "aria-label": "Verification code"
      }
    ),
    error && errorText && /* @__PURE__ */ (0, import_jsx_runtime32.jsx)("p", { role: "alert", className: "text-center text-[13px] font-medium text-destructive", children: errorText }),
    (onResend || (resendIn ?? 0) > 0) && /* @__PURE__ */ (0, import_jsx_runtime32.jsx)("p", { className: "text-center text-sm text-muted-foreground", children: (resendIn ?? 0) > 0 ? /* @__PURE__ */ (0, import_jsx_runtime32.jsxs)(import_jsx_runtime32.Fragment, { children: [
      "Resend in",
      " ",
      /* @__PURE__ */ (0, import_jsx_runtime32.jsxs)("span", { className: "font-semibold tabular-nums text-accent-strong", children: [
        "00:",
        String(resendIn).padStart(2, "0")
      ] })
    ] }) : /* @__PURE__ */ (0, import_jsx_runtime32.jsx)(
      "button",
      {
        type: "button",
        onClick: onResend,
        className: "font-semibold text-accent hover:underline",
        children: "Resend code"
      }
    ) }),
    mobile && /* @__PURE__ */ (0, import_jsx_runtime32.jsx)(
      Keypad,
      {
        className: "mt-2",
        onDigit: (d) => {
          if (value.length < length) {
            const next = value + d;
            onChange(next);
            if (next.length === length) onComplete?.(next);
          }
        },
        onBackspace: () => onChange(value.slice(0, -1))
      }
    )
  ] });
}

// src/components/notifications.tsx
var React17 = __toESM(require("react"), 1);
var import_lucide_react17 = require("lucide-react");
var import_jsx_runtime33 = require("react/jsx-runtime");
var SEGMENTS = [
  { value: "today", label: "Today" },
  { value: "week", label: "This week" },
  { value: "month", label: "This month" }
];
function NotificationRow({
  item,
  onClick
}) {
  return /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
    ListItem,
    {
      leading: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(IconTile, { shape: "circle", className: "size-10 [&_svg]:size-4", children: item.icon ?? /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(import_lucide_react17.Bell, {}) }),
      title: /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("span", { className: "flex items-center gap-2", children: [
        /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "truncate", children: item.title }),
        !item.read && /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "size-2 shrink-0 rounded-full bg-accent", "aria-label": "Unread" })
      ] }),
      subtitle: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "line-clamp-2 whitespace-normal", children: item.body }),
      trailing: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "text-xs text-faint-foreground", children: formatRecency(item.date) }),
      onClick: onClick ? () => onClick(item) : void 0,
      className: "items-start"
    }
  );
}
function filterBySegment(items, segment) {
  return items.filter((n) => recencyOf(n.date) === segment);
}
function NotificationsScreen({
  items,
  onBack,
  onItemClick,
  segment: segmentProp,
  onSegmentChange,
  className
}) {
  const [internal, setInternal] = React17.useState("today");
  const segment = segmentProp ?? internal;
  const setSegment = onSegmentChange ?? setInternal;
  const shown = filterBySegment(items, segment);
  return /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("div", { className: cn("flex h-full flex-col bg-background", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("header", { className: "relative flex items-center justify-center border-b border-border bg-surface px-4 pt-[max(0.875rem,env(safe-area-inset-top))] pb-3.5", children: [
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
        IconButton,
        {
          variant: "outline",
          size: "sm",
          onClick: onBack,
          "aria-label": "Back",
          className: "absolute left-4",
          children: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(import_lucide_react17.ArrowLeft, {})
        }
      ),
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("h2", { className: "text-[17px] font-bold text-foreground", children: "Notifications" })
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("div", { className: "p-4 pb-2", children: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
      SegmentedControl,
      {
        "aria-label": "Recency",
        options: SEGMENTS,
        value: segment,
        onValueChange: (v) => setSegment(v)
      }
    ) }),
    /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("div", { className: "lx-scroll flex-1 overflow-y-auto px-4 pb-[max(1.5rem,env(safe-area-inset-bottom))]", children: shown.length > 0 ? /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(List, { children: shown.map((n) => /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(NotificationRow, { item: n, onClick: onItemClick }, n.id)) }) : /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
      EmptyState,
      {
        className: "mt-6",
        icon: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(import_lucide_react17.Bell, {}),
        title: "Nothing here yet",
        description: "You're all caught up for this period."
      }
    ) })
  ] });
}
function BellPopover({
  items,
  onItemClick,
  onViewAll,
  unreadCount
}) {
  const unread = unreadCount ?? items.filter((n) => !n.read).length;
  const recent = [...items].sort((a, b) => b.date.getTime() - a.date.getTime()).slice(0, 5);
  return /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)(Popover, { children: [
    /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(PopoverTrigger, { asChild: true, children: /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("span", { className: "relative inline-flex", children: [
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(IconButton, { variant: "outline", size: "sm", "aria-label": "Notifications", children: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(import_lucide_react17.Bell, {}) }),
      unread > 0 && /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "pointer-events-none absolute -top-0.5 -right-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-bold text-destructive-foreground", children: unread })
    ] }) }),
    /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)(PopoverContent, { align: "end", className: "w-[380px] p-0", children: [
      /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("div", { className: "flex items-center justify-between border-b border-border px-4 py-3", children: [
        /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("span", { className: "text-sm font-bold text-foreground", children: "Notifications" }),
        unread > 0 && /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("span", { className: "text-xs font-semibold text-muted-foreground", children: [
          unread,
          " unread"
        ] })
      ] }),
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("div", { className: "lx-scroll max-h-[360px] overflow-y-auto px-2 py-1", children: recent.length > 0 ? /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(List, { className: "divide-border/50", children: recent.map((n) => /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(NotificationRow, { item: n, onClick: onItemClick }, n.id)) }) : /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("p", { className: "py-8 text-center text-sm text-muted-foreground", children: "You're all caught up." }) }),
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("div", { className: "border-t border-border p-2", children: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
        "button",
        {
          type: "button",
          onClick: onViewAll,
          className: "flex w-full items-center justify-center rounded-md py-2 text-sm font-semibold text-accent transition-colors hover:bg-accent-soft",
          children: "View all notifications"
        }
      ) })
    ] })
  ] });
}
function NotificationsArchive({
  items,
  onItemClick,
  segment: segmentProp,
  onSegmentChange,
  className
}) {
  const [internal, setInternal] = React17.useState("today");
  const segment = segmentProp ?? internal;
  const setSegment = onSegmentChange ?? setInternal;
  const shown = filterBySegment(items, segment);
  return /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("div", { className: cn("mx-auto flex max-w-[720px] flex-col gap-4", className), children: [
    /* @__PURE__ */ (0, import_jsx_runtime33.jsxs)("div", { className: "flex items-center justify-between gap-4", children: [
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("h2", { className: "text-xl font-bold text-foreground", children: "Notifications" }),
      /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
        SegmentedControl,
        {
          "aria-label": "Recency",
          size: "sm",
          className: "w-[320px]",
          options: SEGMENTS,
          value: segment,
          onValueChange: (v) => setSegment(v)
        }
      )
    ] }),
    /* @__PURE__ */ (0, import_jsx_runtime33.jsx)("div", { className: "rounded-lg border border-border bg-surface px-5 py-2", children: shown.length > 0 ? /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(List, { children: shown.map((n) => /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(NotificationRow, { item: n, onClick: onItemClick }, n.id)) }) : /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(
      EmptyState,
      {
        icon: /* @__PURE__ */ (0, import_jsx_runtime33.jsx)(import_lucide_react17.Bell, {}),
        title: "Nothing here yet",
        description: "You're all caught up for this period.",
        className: "my-4"
      }
    ) })
  ] });
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  AmountText,
  AppShell,
  Avatar,
  Badge,
  BalanceHeader,
  BankCard,
  BellPopover,
  BottomTabBar,
  Button,
  CalendarRangePicker,
  Card,
  CardCarousel,
  CodeEntry,
  CodeInput,
  ConfirmDialog,
  DetailDisclosure,
  Divider,
  Drawer,
  DrawerBody,
  DrawerFooter,
  DrawerHeader,
  EmptyState,
  FilterBar,
  GlobalSearch,
  IconButton,
  IconTile,
  Input,
  Keypad,
  List,
  ListItem,
  Modal,
  ModalBody,
  ModalFooter,
  ModalHeader,
  MoneyList,
  NotificationsArchive,
  NotificationsScreen,
  OptionList,
  PlatformProvider,
  PlatformSwitch,
  Popover,
  PopoverContent,
  PopoverTrigger,
  PrimaryAction,
  QuickActions,
  ResponsiveDialog,
  SearchInput,
  SectionHeader,
  SegmentedControl,
  Sheet,
  SheetBody,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  Sidebar,
  Skeleton,
  SkeletonRow,
  Spinner,
  Stat,
  StatPair,
  Switch,
  ViewAllChip,
  Wizard,
  avatarVariants,
  badgeVariants,
  buttonVariants,
  cardVariants,
  cn,
  downloadCsv,
  formatBalance,
  formatMoney,
  formatRecency,
  iconButtonVariants,
  isFilterActive,
  monthBandLabel,
  recencyOf,
  useMediaQuery,
  usePlatform
});
//# sourceMappingURL=index.cjs.map