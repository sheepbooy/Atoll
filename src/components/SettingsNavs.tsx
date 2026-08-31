import {
  ReactNode,
} from "react";
import {
  ArrowLeft,
  Settings2,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";

export function SettingsPageNav({
  onBack,
  backLabel,
  icon,
  title,
}: {
  onBack: () => void;
  backLabel: string;
  icon: ReactNode;
  title: string;
}) {
  return (
    <div className="settings-subview-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>{backLabel}</span>
      </button>
      <span className="settings-header-title">
        {icon}
        <span>{title}</span>
      </span>
    </div>
  );
}

export interface SettingsSubviewNavProps {
  onBack: () => void;
}

export function SettingsSubviewNav({ onBack }: SettingsSubviewNavProps) {
  const { t } = useTranslation();

  return (
    <div className="settings-subview-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>{t("nav.back")}</span>
      </button>
      <span className="settings-header-title">
        <Settings2 size={14} />
        <span>{t("nav.settings")}</span>
      </span>
    </div>
  );
}

