import { ClassValue } from 'clsx';
import * as React from 'react';
import * as class_variance_authority_types from 'class-variance-authority/types';
import { VariantProps } from 'class-variance-authority';
import * as SwitchPrimitive from '@radix-ui/react-switch';
import * as PopoverPrimitive from '@radix-ui/react-popover';
import { DateRange } from 'react-day-picker';
export { DateRange } from 'react-day-picker';

declare function cn(...inputs: ClassValue[]): string;

/** Money + number formatting helpers used across LayerX UI. */
interface FormatMoneyOptions {
    /** ISO-style currency symbol/code shown after the amount, e.g. "USD". */
    currency?: string;
    /** Force a leading + for positive values. Default true when `signed`. */
    signed?: boolean;
    /** Fraction digits. Default 2. */
    decimals?: number;
    /** Symbol prepended to the number. Default "$". Pass "" for none. */
    symbol?: string;
}
declare function formatMoney(value: number, opts?: FormatMoneyOptions): string;
/** "$23,043.00" — unsigned, for balances. */
declare function formatBalance(value: number, symbol?: string): string;
/** Compact "30m ago" / "2h ago" style recency labels. */
declare function formatRecency(date: Date, now?: Date): string;
/** Group key for month bands, e.g. "February 2025". */
declare function monthBandLabel(date: Date): string;
/** Build a CSV string from rows of primitive cells and trigger a download. */
declare function downloadCsv(filename: string, header: string[], rows: (string | number)[][]): void;

type Platform = "mobile" | "desktop";
type PlatformSetting = Platform | "auto";
/**
 * Controls how LayerX responsive patterns resolve their mobile/desktop
 * variants. "auto" (default) follows the viewport (mobile = <768px).
 * Wrap demos in a fixed value to force a variant regardless of viewport —
 * e.g. inside a phone frame on a desktop docs page.
 */
declare function PlatformProvider({ value, children, }: {
    value?: PlatformSetting;
    children: React.ReactNode;
}): React.JSX.Element;
/** SSR-safe media query hook (mobile = viewport < 768px). */
declare function useMediaQuery(query: string): boolean;
/**
 * Resolve the current platform. Priority:
 * explicit prop > nearest PlatformProvider > viewport media query.
 */
declare function usePlatform(override?: PlatformSetting): Platform;
/** Renders one of two branches by platform. */
declare function PlatformSwitch({ mobile, desktop, platform, }: {
    mobile: React.ReactNode;
    desktop: React.ReactNode;
    platform?: PlatformSetting;
}): React.JSX.Element;

/** One row of money movement (transaction, earning, agent spend…). */
interface MoneyItem {
    id: string;
    title: string;
    subtitle?: string;
    /** Signed amount in major units. */
    amount: number;
    currency?: string;
    status?: string;
    /** Used for month bands + sort. */
    date: Date;
    /** Leading visual: Avatar, IconTile, flag… */
    leading?: React.ReactNode;
}
/** A month band with its rows and subtotal. */
interface MoneyGroup {
    id: string;
    label: string;
    subtotal: number;
    currency?: string;
    items: MoneyItem[];
}
interface NotificationItem {
    id: string;
    title: string;
    body: string;
    /** When it happened — drives recency segments + labels. */
    date: Date;
    icon?: React.ReactNode;
    read?: boolean;
    href?: string;
}
/** Bucket a notification lands in for the recency segments. */
type RecencySegment = "today" | "week" | "month";
declare function recencyOf(date: Date, now?: Date): RecencySegment;

declare const buttonVariants: (props?: ({
    variant?: "link" | "primary" | "secondary" | "soft" | "accent" | "destructive" | "ghost" | null | undefined;
    size?: "sm" | "md" | "lg" | "icon" | null | undefined;
    fullWidth?: boolean | null | undefined;
} & class_variance_authority_types.ClassProp) | undefined) => string;
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
    asChild?: boolean;
    loading?: boolean;
}
declare const Button: React.ForwardRefExoticComponent<ButtonProps & React.RefAttributes<HTMLButtonElement>>;

declare const iconButtonVariants: (props?: ({
    variant?: "primary" | "soft" | "accent" | "ghost" | "outline" | null | undefined;
    size?: "sm" | "md" | "lg" | null | undefined;
} & class_variance_authority_types.ClassProp) | undefined) => string;
interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof iconButtonVariants> {
}
declare const IconButton: React.ForwardRefExoticComponent<IconButtonProps & React.RefAttributes<HTMLButtonElement>>;

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
    error?: boolean;
    /** Optional leading adornment (icon, flag, "+1" etc). */
    leading?: React.ReactNode;
    trailing?: React.ReactNode;
}
declare const Input: React.ForwardRefExoticComponent<InputProps & React.RefAttributes<HTMLInputElement>>;

interface SearchInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
    onClear?: () => void;
}
/** Pill search field, as used in the home header and asset list. */
declare const SearchInput: React.ForwardRefExoticComponent<SearchInputProps & React.RefAttributes<HTMLInputElement>>;

declare const badgeVariants: (props?: ({
    variant?: "accent" | "destructive" | "outline" | "neutral" | "success" | "warning" | null | undefined;
    size?: "sm" | "md" | null | undefined;
} & class_variance_authority_types.ClassProp) | undefined) => string;
interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {
}
declare function Badge({ className, variant, size, ...props }: BadgeProps): React.JSX.Element;

declare const avatarVariants: (props?: ({
    size?: "sm" | "md" | "lg" | "xs" | "xl" | null | undefined;
    tone?: "primary" | "accent" | "neutral" | null | undefined;
} & class_variance_authority_types.ClassProp) | undefined) => string;
interface AvatarProps extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof avatarVariants> {
    src?: string;
    alt?: string;
    /** Fallback initials; derived from `alt` when omitted. */
    initials?: string;
}
declare const Avatar: React.ForwardRefExoticComponent<AvatarProps & React.RefAttributes<HTMLSpanElement>>;

declare const cardVariants: (props?: ({
    elevation?: "flat" | "outline" | "raised" | null | undefined;
    padding?: "sm" | "md" | "lg" | "none" | null | undefined;
} & class_variance_authority_types.ClassProp) | undefined) => string;
interface CardProps extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof cardVariants> {
    asChild?: boolean;
}
declare const Card: React.ForwardRefExoticComponent<CardProps & React.RefAttributes<HTMLDivElement>>;

/** iOS-style switch used across settings sheets in the design set. */
declare const Switch: React.ForwardRefExoticComponent<Omit<SwitchPrimitive.SwitchProps & React.RefAttributes<HTMLButtonElement>, "ref"> & React.RefAttributes<HTMLButtonElement>>;

interface SegmentedControlOption {
    value: string;
    label: React.ReactNode;
}
interface SegmentedControlProps {
    options: SegmentedControlOption[];
    value: string;
    onValueChange: (value: string) => void;
    className?: string;
    size?: "sm" | "md";
    "aria-label"?: string;
}
/**
 * Gray track + white active thumb — the "Today / This week / This month"
 * recency switcher and "All / Fiat / Crypto" filter in the design set.
 */
declare function SegmentedControl({ options, value, onValueChange, className, size, ...aria }: SegmentedControlProps): React.JSX.Element;

/** Vertical stack of rows with hairline dividers between them. */
declare function List({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;
/** Soft rounded-square icon container (quick actions, list leading icons). */
declare function IconTile({ className, tone, shape, ...props }: React.HTMLAttributes<HTMLSpanElement> & {
    tone?: "neutral" | "accent" | "success" | "destructive";
    shape?: "square" | "circle";
}): React.JSX.Element;
interface ListItemProps extends Omit<React.HTMLAttributes<HTMLDivElement>, "title"> {
    /** Avatar, IconTile, flag, or any leading node. */
    leading?: React.ReactNode;
    title: React.ReactNode;
    subtitle?: React.ReactNode;
    /** Amount, Badge, Switch, chevron… right-aligned. */
    trailing?: React.ReactNode;
    /** Adds a chevron and pointer cursor. */
    navigates?: boolean;
    /** Text under the trailing node (e.g. timestamp). */
    trailingCaption?: React.ReactNode;
}
declare function ListItem({ className, leading, title, subtitle, trailing, trailingCaption, navigates, onClick, ...props }: ListItemProps): React.JSX.Element;
declare function SectionHeader({ title, action, className, }: {
    title: React.ReactNode;
    /** e.g. a "View all" chip button. */
    action?: React.ReactNode;
    className?: string;
}): React.JSX.Element;
/** Small pill button used for "View all" actions in section headers. */
declare function ViewAllChip({ className, children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>): React.JSX.Element;
declare function Divider({ className }: {
    className?: string;
}): React.JSX.Element;

interface AmountTextProps extends React.HTMLAttributes<HTMLSpanElement> {
    value: number;
    currency?: string;
    decimals?: number;
    /** "$" by default; pass "" to hide. */
    symbol?: string;
    /**
     * "signed" — positive renders success green with +, negative destructive red.
     * "neutral" — always foreground, sign still shown.
     */
    colorMode?: "signed" | "neutral";
}
/** Signed money text with tabular figures, colored by sign. */
declare function AmountText({ value, currency, decimals, symbol, colorMode, className, ...props }: AmountTextProps): React.JSX.Element;

/** Big number over a muted label — "18 / Total referrals". */
declare function Stat({ value, label, className, align, }: {
    value: React.ReactNode;
    label: React.ReactNode;
    className?: string;
    align?: "left" | "center";
}): React.JSX.Element;
/** Two stats separated by a vertical hairline, as on Earning / Network. */
declare function StatPair({ left, right, className, }: {
    left: {
        value: React.ReactNode;
        label: React.ReactNode;
    };
    right: {
        value: React.ReactNode;
        label: React.ReactNode;
    };
    className?: string;
}): React.JSX.Element;

/**
 * Centered empty state — soft circular icon well, title, copy, optional CTA.
 * ("Ready to get started?" from the design set.)
 */
declare function EmptyState({ icon, title, description, action, className, }: {
    icon?: React.ReactNode;
    title: React.ReactNode;
    description?: React.ReactNode;
    action?: React.ReactNode;
    className?: string;
}): React.JSX.Element;

declare function Spinner({ className }: {
    className?: string;
}): React.JSX.Element;
declare function Skeleton({ className }: {
    className?: string;
}): React.JSX.Element;
/** List-row shaped skeleton for loading lists. */
declare function SkeletonRow(): React.JSX.Element;

interface OptionListItem {
    value: string;
    label: React.ReactNode;
    description?: React.ReactNode;
}
/**
 * Right-aligned radio rows — the filter sheet option list
 * ("All time / Today / Last 7 days…") from the design set.
 */
declare function OptionList({ items, value, onValueChange, className, "aria-label": ariaLabel, }: {
    items: OptionListItem[];
    value: string;
    onValueChange: (value: string) => void;
    className?: string;
    "aria-label"?: string;
}): React.JSX.Element;

interface BalanceHeaderProps {
    label?: string;
    value: number;
    symbol?: string;
    /** Daily change info line, e.g. { amount: "$234", percent: "+0.81%", up: true } */
    change?: {
        text: string;
        up: boolean;
    };
    hidden?: boolean;
    onHiddenChange?: (hidden: boolean) => void;
    align?: "left" | "center";
    className?: string;
}
/**
 * Big balance with privacy eye toggle — the home/wallet header
 * in the design set ("$ 23,043.00" + 1-day change).
 */
declare function BalanceHeader({ label, value, symbol, change, hidden: hiddenProp, onHiddenChange, align, className, }: BalanceHeaderProps): React.JSX.Element;

interface QuickAction {
    id: string;
    label: string;
    icon: React.ReactNode;
}
/**
 * Circle icon + label grid ("Send / Receive / Swap / Pay bills").
 * Circles use a hairline border on white, per the design set.
 */
declare function QuickActions({ actions, onAction, className, }: {
    actions: QuickAction[];
    onAction?: (id: string) => void;
    className?: string;
}): React.JSX.Element;

interface BankCardData {
    holder: string;
    /** Masked number, e.g. "6464 XXXX XXXX 9980". */
    number: string;
    kind?: string;
    balanceLabel?: string;
    balance?: string;
    expiry?: string;
    brand?: string;
    status?: {
        label: string;
        tone?: "success" | "neutral" | "destructive";
    };
    theme?: "light" | "dark";
}
/** Payment-card visual from the wallet/cards screens. */
declare function BankCard({ data, className }: {
    data: BankCardData;
    className?: string;
}): React.JSX.Element;
/** Horizontal card pager with dot indicator, as on the Cards screen. */
declare function CardCarousel({ cards, renderCard, className, }: {
    cards: BankCardData[];
    renderCard?: (card: BankCardData, index: number) => React.ReactNode;
    className?: string;
}): React.JSX.Element;

interface SheetProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    children: React.ReactNode;
    /** Portal target — pass a phone-frame ref in docs to contain the sheet. */
    portalContainer?: HTMLElement | null;
}
/**
 * Bottom sheet — the mobile overlay of the design set: drag handle,
 * rounded top corners, slides up over a dimmed page.
 * Esc, overlay tap, and the close affordance all dismiss (focus-trapped).
 */
declare function Sheet({ open, onOpenChange, children, portalContainer }: SheetProps): React.JSX.Element;
/** Top grab handle + optional centered title row. */
declare function SheetHeader({ title, className, children, }: {
    title?: React.ReactNode;
    className?: string;
    children?: React.ReactNode;
}): React.JSX.Element;
declare function SheetDescription({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>): React.JSX.Element;
declare function SheetBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;
/**
 * Stacked/paired CTAs pinned to the sheet bottom.
 * Two children render side-by-side (Clear | Apply), one renders full-width.
 */
declare function SheetFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;

interface ModalProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    children: React.ReactNode;
    portalContainer?: HTMLElement | null;
    className?: string;
}
/**
 * Desktop confirmation modal: centered, max 440px, rounded, Esc + overlay
 * dismiss, focus-trapped (Radix Dialog).
 */
declare function Modal({ open, onOpenChange, children, portalContainer, className }: ModalProps): React.JSX.Element;
declare function ModalHeader({ title, description, onClose, className, }: {
    title: React.ReactNode;
    description?: React.ReactNode;
    onClose?: () => void;
    className?: string;
}): React.JSX.Element;
declare function ModalBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;
declare function ModalFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;

interface DrawerProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    children: React.ReactNode;
    portalContainer?: HTMLElement | null;
    /** Width of the right-side panel. Default 420px. */
    width?: number | string;
}
/**
 * Right-side drawer — the desktop detail/education surface.
 * Esc + overlay dismiss, focus-trapped.
 */
declare function Drawer({ open, onOpenChange, children, portalContainer, width }: DrawerProps): React.JSX.Element;
declare function DrawerHeader({ title, description, onClose, className, }: {
    title: React.ReactNode;
    description?: React.ReactNode;
    onClose?: () => void;
    className?: string;
}): React.JSX.Element;
declare function DrawerBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;
declare function DrawerFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element;

/**
 * One overlay, two bodies: bottom sheet on mobile, centered 440px modal on
 * desktop. This is the LayerX confirmation/detail contract from the pattern
 * table — consequence copy included, Esc/overlay dismiss, focus-trapped.
 */
declare function ResponsiveDialog({ open, onOpenChange, title, description, children, footer, platform, portalContainer, }: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    title: React.ReactNode;
    /** Consequence copy — shown under the title. */
    description?: React.ReactNode;
    children?: React.ReactNode;
    footer?: React.ReactNode;
    platform?: PlatformSetting;
    portalContainer?: HTMLElement | null;
}): React.JSX.Element;
interface ConfirmAction {
    label: React.ReactNode;
    onClick?: () => void;
    variant?: ButtonProps["variant"];
    loading?: boolean;
}
/**
 * Ready-made confirm: icon/title, consequence copy, and paired actions
 * (secondary | primary). Renders as a bottom sheet on mobile, a centered
 * modal on desktop.
 */
declare function ConfirmDialog({ open, onOpenChange, icon, title, consequence, confirm, cancel, platform, portalContainer, }: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    icon?: React.ReactNode;
    title: React.ReactNode;
    /** The "what happens if you do this" copy. Required — it's the point. */
    consequence: React.ReactNode;
    confirm: ConfirmAction;
    cancel?: ConfirmAction;
    platform?: PlatformSetting;
    portalContainer?: HTMLElement | null;
}): React.JSX.Element;

declare const Popover: React.FC<PopoverPrimitive.PopoverProps>;
declare const PopoverTrigger: React.ForwardRefExoticComponent<PopoverPrimitive.PopoverTriggerProps & React.RefAttributes<HTMLButtonElement>>;
/** Anchored popover surface — desktop filter chips, bell menu, etc. */
declare const PopoverContent: React.ForwardRefExoticComponent<Omit<PopoverPrimitive.PopoverContentProps & React.RefAttributes<HTMLDivElement>, "ref"> & React.RefAttributes<HTMLDivElement>>;

/**
 * Calendar range picker in an anchored popover — desktop companion to the
 * "From date / To date" selects in the mobile filter sheet.
 */
declare function CalendarRangePicker({ value, onChange, placeholder, className, }: {
    value?: DateRange;
    onChange?: (range: DateRange | undefined) => void;
    placeholder?: string;
    className?: string;
}): React.JSX.Element;

interface CodeInputProps {
    length?: number;
    value: string;
    onChange: (value: string) => void;
    onComplete?: (value: string) => void;
    error?: boolean;
    disabled?: boolean;
    autoFocus?: boolean;
    /** Hide the native input — pair with the on-screen Keypad. */
    readOnly?: boolean;
    className?: string;
    "aria-label"?: string;
}
/**
 * Segmented code entry: per-box display, full-code paste, auto-advance,
 * red error state — the 2FA/PIN kit from the design set.
 * A single hidden input drives everything, so paste and IME work everywhere.
 */
declare function CodeInput({ length, value, onChange, onComplete, error, disabled, autoFocus, readOnly, className, ...aria }: CodeInputProps): React.JSX.Element;
/**
 * On-screen numeric keypad (1–9 with T9 letters, "+*#", 0, backspace) —
 * the mobile half of the code-entry kit.
 */
declare function Keypad({ onDigit, onBackspace, className, }: {
    onDigit: (digit: string) => void;
    onBackspace: () => void;
    className?: string;
}): React.JSX.Element;

interface NavItem {
    id: string;
    label: string;
    icon?: React.ReactNode;
    /** Numeric badge (e.g. pending approvals on Activity). */
    badge?: number;
}
/**
 * Mobile navigation: bottom tab bar (Home, Agents, Activity, More) with a
 * raised center action button at thumb reach.
 */
declare function BottomTabBar({ items, activeId, onNavigate, onFab, fabIcon, className, }: {
    items: NavItem[];
    activeId: string;
    onNavigate?: (id: string) => void;
    onFab?: () => void;
    fabIcon?: React.ReactNode;
    className?: string;
}): React.JSX.Element;
/**
 * Desktop navigation: left sidebar with product nav (badge support for
 * approvals), an Explorer link, and a settings footer.
 */
declare function Sidebar({ items, activeId, onNavigate, logo, explorerLabel, onExplorer, settingsLabel, onSettings, user, className, }: {
    items: NavItem[];
    activeId: string;
    onNavigate?: (id: string) => void;
    logo?: React.ReactNode;
    explorerLabel?: string;
    onExplorer?: () => void;
    settingsLabel?: string;
    onSettings?: () => void;
    user?: {
        name: string;
        subtitle?: string;
        avatarSrc?: string;
    };
    className?: string;
}): React.JSX.Element;
interface AppShellProps {
    nav: NavItem[];
    activeNav: string;
    onNavigate?: (id: string) => void;
    /** Center action — mobile FAB / desktop header button. */
    onPrimaryAction?: () => void;
    primaryActionLabel?: string;
    primaryActionIcon?: React.ReactNode;
    /** Header slots */
    user?: {
        name: string;
        initials?: string;
        avatarSrc?: string;
    };
    onSearch?: () => void;
    onNotifications?: () => void;
    notificationCount?: number;
    /** Desktop sidebar extras */
    onExplorer?: () => void;
    onSettings?: () => void;
    logo?: React.ReactNode;
    /** Page title shown in the desktop header. */
    title?: React.ReactNode;
    /** Desktop header actions (page-level buttons). */
    headerActions?: React.ReactNode;
    platform?: PlatformSetting;
    className?: string;
    children: React.ReactNode;
}
/**
 * The responsive app frame.
 * Mobile → header (avatar, search, bell) + content + bottom tab bar w/ FAB.
 * Desktop → left sidebar (approval badge, Explorer, settings footer) +
 * top header (title, search field, bell, avatar) + content.
 */
declare function AppShell({ nav, activeNav, onNavigate, onPrimaryAction, primaryActionLabel, primaryActionIcon, user, onSearch, onNotifications, notificationCount, onExplorer, onSettings, logo, title, headerActions, platform, className, children, }: AppShellProps): React.JSX.Element;

/**
 * The screen's one primary CTA, placed per platform contract:
 * - mobile: full-width pill pinned at thumb reach (sticky footer with a
 *   soft gradient scrim above the tab bar / home indicator)
 * - desktop: fixed-width button — render it in the pane footer
 *   (`position="footer"`) or pass it to AppShell `headerActions`
 *   (`position="header"`).
 */
declare function PrimaryAction({ children, platform, position, className, ...props }: ButtonProps & {
    platform?: PlatformSetting;
    position?: "footer" | "header";
}): React.JSX.Element;

/**
 * Detail / education surface, per platform:
 * - mobile:  bottom sheet (variant="sheet", default) or a pushed full screen
 *            (variant="pushed") with a back header
 * - desktop: right-side drawer (variant="drawer", default) or an inline
 *            expanding section (variant="inline")
 */
declare function DetailDisclosure({ open, onOpenChange, title, children, mobileVariant, desktopVariant, platform, portalContainer, summary, }: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    title: React.ReactNode;
    children: React.ReactNode;
    mobileVariant?: "sheet" | "pushed";
    desktopVariant?: "drawer" | "inline";
    platform?: PlatformSetting;
    portalContainer?: HTMLElement | null;
    /** For inline variant: the always-visible summary row content. */
    summary?: React.ReactNode;
}): React.JSX.Element | null;

interface FilterDef {
    id: string;
    label: string;
    type: "options" | "date-range";
    options?: {
        value: string;
        label: string;
    }[];
}
type FilterValues = Record<string, string | DateRange | undefined>;
declare function isFilterActive(v: FilterValues[string]): boolean;
/**
 * Filters, per platform:
 * - mobile:  a Filter button opens a sheet with every filter stacked,
 *            Clear + Apply footer (Apply commits a draft state)
 * - desktop: each filter is a chip; its editor is a popover anchored to
 *            the chip (option lists or a calendar range picker)
 */
declare function FilterBar({ filters, values, onChange, platform, portalContainer, className, }: {
    filters: FilterDef[];
    values: FilterValues;
    onChange: (values: FilterValues) => void;
    platform?: PlatformSetting;
    portalContainer?: HTMLElement | null;
    className?: string;
}): React.JSX.Element;

/**
 * Lists of money, per platform:
 * - mobile:  stacked rows under month bands with subtotals
 * - desktop: a true table — sortable columns, hover states, sticky group
 *            rows, CSV export
 */
declare function MoneyList({ groups, onItemClick, platform, exportName, className, }: {
    groups: MoneyGroup[];
    onItemClick?: (item: MoneyItem) => void;
    platform?: PlatformSetting;
    exportName?: string;
    className?: string;
}): React.JSX.Element;

interface WizardStep {
    id: string;
    /** Short label for the progress rail / summary. */
    label: string;
    /** The one decision this screen asks for. */
    title: string;
    description?: string;
    render: () => React.ReactNode;
    /** Gate Continue until the step is valid. */
    canContinue?: () => boolean;
}
interface WizardSummaryItem {
    label: string;
    value: React.ReactNode;
}
/**
 * Multi-step journeys, per platform:
 * - mobile:  full-screen wizard — one decision per screen, progress bar,
 *            back button, pinned Continue at thumb reach
 * - desktop: split pane — form on the left, a live "what will happen"
 *            summary pinned on the right
 */
declare function Wizard({ steps, summary, onComplete, onCancel, completeLabel, summaryTitle, platform, className, }: {
    steps: WizardStep[];
    /** Live summary of the choices so far (desktop right rail). */
    summary?: WizardSummaryItem[];
    onComplete?: () => void;
    onCancel?: () => void;
    completeLabel?: string;
    summaryTitle?: string;
    platform?: PlatformSetting;
    className?: string;
}): React.JSX.Element;

interface SearchResultItem {
    id: string;
    title: string;
    subtitle?: string;
    icon?: React.ReactNode;
    /** Extra match terms. */
    keywords?: string[];
}
interface SearchResultGroup {
    id: string;
    label: string;
    items: SearchResultItem[];
}
/**
 * Search, per platform:
 * - mobile:  a pushed full-screen search page with autofocus + recents
 * - desktop: a global command bar (binds Cmd+K / Ctrl+K) with type-ahead
 */
declare function GlobalSearch({ open, onOpenChange, groups, onSelect, recents, placeholder, enableHotkey, platform, portalContainer, }: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    groups: SearchResultGroup[];
    onSelect?: (item: SearchResultItem) => void;
    recents?: SearchResultItem[];
    placeholder?: string;
    /** Bind Cmd+K / Ctrl+K to open. Desktop only. Default true. */
    enableHotkey?: boolean;
    platform?: PlatformSetting;
    portalContainer?: HTMLElement | null;
}): React.JSX.Element;

/**
 * Code / secret entry, per platform:
 * - mobile:  tap-per-box code kit — segmented display driven by the
 *            on-screen Keypad (native keyboard stays down)
 * - desktop: a single segmented input with full-code paste + auto-advance
 *
 * Includes the resend timer row and error copy from the 2FA screens.
 */
declare function CodeEntry({ length, value, onChange, onComplete, error, errorText, resendIn, onResend, platform, className, }: {
    length?: number;
    value: string;
    onChange: (value: string) => void;
    onComplete?: (value: string) => void;
    error?: boolean;
    errorText?: string;
    /** Seconds until resend is allowed; 0/undefined = resend available. */
    resendIn?: number;
    onResend?: () => void;
    platform?: PlatformSetting;
    className?: string;
}): React.JSX.Element;

declare function NotificationsScreen({ items, onBack, onItemClick, segment: segmentProp, onSegmentChange, className, }: {
    items: NotificationItem[];
    onBack?: () => void;
    onItemClick?: (item: NotificationItem) => void;
    segment?: RecencySegment;
    onSegmentChange?: (s: RecencySegment) => void;
    className?: string;
}): React.JSX.Element;
declare function BellPopover({ items, onItemClick, onViewAll, unreadCount, }: {
    items: NotificationItem[];
    onItemClick?: (item: NotificationItem) => void;
    onViewAll?: () => void;
    unreadCount?: number;
}): React.JSX.Element;
/** Full archive page (desktop counterpart to the mobile pushed screen). */
declare function NotificationsArchive({ items, onItemClick, segment: segmentProp, onSegmentChange, className, }: {
    items: NotificationItem[];
    onItemClick?: (item: NotificationItem) => void;
    segment?: RecencySegment;
    onSegmentChange?: (s: RecencySegment) => void;
    className?: string;
}): React.JSX.Element;

export { AmountText, type AmountTextProps, AppShell, type AppShellProps, Avatar, type AvatarProps, Badge, type BadgeProps, BalanceHeader, type BalanceHeaderProps, BankCard, type BankCardData, BellPopover, BottomTabBar, Button, type ButtonProps, CalendarRangePicker, Card, CardCarousel, type CardProps, CodeEntry, CodeInput, type CodeInputProps, type ConfirmAction, ConfirmDialog, DetailDisclosure, Divider, Drawer, DrawerBody, DrawerFooter, DrawerHeader, type DrawerProps, EmptyState, FilterBar, type FilterDef, type FilterValues, type FormatMoneyOptions, GlobalSearch, IconButton, IconTile, Input, type InputProps, Keypad, List, ListItem, type ListItemProps, Modal, ModalBody, ModalFooter, ModalHeader, type ModalProps, type MoneyGroup, type MoneyItem, MoneyList, type NavItem, type NotificationItem, NotificationsArchive, NotificationsScreen, OptionList, type OptionListItem, type Platform, PlatformProvider, type PlatformSetting, PlatformSwitch, Popover, PopoverContent, PopoverTrigger, PrimaryAction, type QuickAction, QuickActions, type RecencySegment, ResponsiveDialog, SearchInput, type SearchInputProps, type SearchResultGroup, type SearchResultItem, SectionHeader, SegmentedControl, type SegmentedControlOption, type SegmentedControlProps, Sheet, SheetBody, SheetDescription, SheetFooter, SheetHeader, type SheetProps, Sidebar, Skeleton, SkeletonRow, Spinner, Stat, StatPair, Switch, ViewAllChip, Wizard, type WizardStep, type WizardSummaryItem, avatarVariants, badgeVariants, buttonVariants, cardVariants, cn, downloadCsv, formatBalance, formatMoney, formatRecency, iconButtonVariants, isFilterActive, monthBandLabel, recencyOf, useMediaQuery, usePlatform };
