import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/common/primitives";

type Guard = { dirty: boolean; saving: boolean; onDiscard?: () => void };
type Navigation = { register: (guard: Guard | null) => void; finishDraft: () => void; requestLeave: (leave: () => void) => void };
const Context = createContext<Navigation>({ register: () => undefined, finishDraft: () => undefined, requestLeave: (leave) => leave() });

export function DraftNavigationProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const guard = useRef<Guard | null>(null);
  const [pending, setPending] = useState<(() => void) | null>(null);
  const [blocked, setBlocked] = useState(false);
  const navigation: Navigation = {
    register: (value) => { guard.current = value; },
    finishDraft: () => { guard.current = null; },
    requestLeave: (leave) => {
      if (guard.current?.saving) { setBlocked(true); return; }
      if (guard.current?.dirty) setPending(() => leave);
      else leave();
    },
  };
  return <Context.Provider value={navigation}>{children}
    {blocked && <Dialog title={t("common.saving")} onClose={() => setBlocked(false)}><p>{t("m8.waitSaving")}</p><button className="button button-secondary" onClick={() => setBlocked(false)}>{t("common.close")}</button></Dialog>}
    {pending && <Dialog title={t("m8.discardTitle")} onClose={() => setPending(null)}><p>{t("m8.discardBody")}</p><div className="settings-actions"><button className="button button-secondary" onClick={() => setPending(null)}>{t("m8.stay")}</button><button className="button button-danger" onClick={() => { const discarded = guard.current; guard.current = null; discarded?.onDiscard?.(); setPending(null); pending(); }}>{t("m8.discard")}</button></div></Dialog>}
  </Context.Provider>;
}

export function useDraftNavigation() { return useContext(Context); }
export function useDraftGuard(guard: Guard) {
  const navigation = useDraftNavigation();
  useEffect(() => { navigation.register(guard); return () => navigation.register(null); }, [navigation.register, guard.dirty, guard.saving, guard.onDiscard]);
  return navigation.requestLeave;
}
