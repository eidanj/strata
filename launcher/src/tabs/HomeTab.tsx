import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  errorText,
  inTauri,
  type AccountsView,
  type AuthStatus,
  type DeviceCode,
  type GameExit,
  type LaunchProgress,
  type Overview,
  type Profile,
  type VersionOption,
} from "../lib/api";
import { useNotify } from "../lib/notices";
import { SkinView } from "../components/SkinView";
import type { LayerId } from "../layers";

type Phase = "idle" | "launching" | "running";

interface Props {
  overview: Overview | null;
  onChanged: () => void;
  onNavigate: (id: LayerId) => void;
  onAccount: (name: string | null) => void;
}

export function HomeTab({ overview, onChanged, onNavigate, onAccount }: Props) {
  const notify = useNotify();
  const [accounts, setAccounts] = useState<AccountsView | null>(null);
  const [versions, setVersions] = useState<VersionOption[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<LaunchProgress | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [showLog, setShowLog] = useState(false);
  const [code, setCode] = useState<DeviceCode | null>(null);
  const [signingIn, setSigningIn] = useState<string | null>(null);

  const inst = overview?.instance;
  const active = accounts?.accounts.find((a) => a.active) ?? null;

  const loadAccounts = useCallback(async () => {
    try {
      const view = await api.accounts();
      setAccounts(view);
      onAccount(view.accounts.find((a) => a.active)?.name ?? null);
    } catch (e) {
      notify("error", "Couldn't read accounts", [errorText(e)]);
    }
  }, [notify, onAccount]);

  useEffect(() => {
    loadAccounts();
    api.versions().then(setVersions).catch(() => setVersions([]));
    api.profiles().then(setProfiles).catch(() => {});
    api.gameStatus().then((s) => {
      setLog(s.log);
      setPhase(s.running ? "running" : s.launching ? "launching" : "idle");
    });
  }, [loadAccounts]);

  // Live events from the backend.
  useEffect(() => {
    if (!inTauri) return;
    const subs = [
      listen<AuthStatus>("auth-status", ({ payload }) => {
        if (payload.state === "signing-in") setSigningIn(payload.message ?? "Signing in");
        if (payload.state === "done") {
          setCode(null);
          setSigningIn(null);
          notify("ok", payload.message ?? "Signed in");
          loadAccounts();
        }
        if (payload.state === "failed") {
          setCode(null);
          setSigningIn(null);
          notify("error", "Sign-in didn't finish", payload.message ? [payload.message] : []);
        }
        if (payload.state === "cancelled") {
          setCode(null);
          setSigningIn(null);
        }
      }),
      listen<LaunchProgress>("launch-progress", ({ payload }) => {
        setProgress(payload);
        setPhase(payload.stage === "running" ? "running" : "launching");
      }),
      listen<string>("game-log", ({ payload }) => setLog((l) => (l.length > 600 ? [...l.slice(-500), payload] : [...l, payload]))),
      listen<GameExit>("game-exit", ({ payload }) => {
        setPhase("idle");
        setProgress(null);
        if (payload.crashed) {
          setShowLog(true);
          notify("error", `Minecraft closed with an error (code ${payload.code})`, ["The game log below shows what happened."]);
        }
      }),
    ];
    return () => {
      subs.forEach((s) => s.then((f) => f()));
    };
  }, [notify, loadAccounts]);

  async function launch() {
    setPhase("launching");
    setProgress({ stage: "account", label: "Checking your account", done: 0, total: 0 });
    try {
      await api.launch();
    } catch (e) {
      setPhase("idle");
      setProgress(null);
      notify("error", "Couldn't start Minecraft", [errorText(e)]);
    }
  }

  async function changeVersion(version: string) {
    try {
      await api.setVersion(version);
      onChanged();
      notify("ok", `Switched to Minecraft ${version}`, ["Mods without a build for this version are marked on the Mods tab."]);
    } catch (e) {
      notify("error", "Couldn't switch versions", [errorText(e)]);
    }
  }

  async function beginSignIn() {
    try {
      setCode(await api.beginSignIn());
    } catch (e) {
      notify("error", "Couldn't start sign-in", [errorText(e)]);
    }
  }

  const profileName = profiles.find((p) => p.id === inst?.activeProfile)?.name ?? inst?.activeProfile ?? "";
  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : null;
  const version = inst?.minecraftVersion ?? "26.3";

  return (
    <>
      <div className="stage__body home">
        <div className="home__figure">
          <SkinView src={active?.skin ?? null} slim={active?.skinVariant === "slim"} label={active ? `${active.name}'s skin` : undefined} />
          <div className="home__ground" aria-hidden="true">
            <span style={{ height: 10 }} />
            <span style={{ height: 22 }} />
            <span style={{ height: 8 }} />
            <span style={{ height: 30 }} />
          </div>
        </div>

        <div className="home__main">
          {active ? (
            <AccountHeader view={accounts!} onChanged={loadAccounts} onAdd={beginSignIn} />
          ) : (
            <h1 className="home__name">Sign in to play</h1>
          )}

          {!active && accounts && (
            <SignIn
              view={accounts}
              code={code}
              signingIn={signingIn}
              onBegin={beginSignIn}
              onSaved={loadAccounts}
            />
          )}
          {active && code && <SignIn view={accounts!} code={code} signingIn={signingIn} onBegin={beginSignIn} onSaved={loadAccounts} />}

          <dl className="home__facts">
            <div>
              <dt>Version</dt>
              <dd>
                <select
                  className="field"
                  value={version}
                  disabled={phase !== "idle"}
                  onChange={(e) => changeVersion(e.target.value)}
                  aria-label="Minecraft version"
                >
                  {(versions.some((v) => v.id === version) ? versions : [{ id: version, installed: true }, ...versions]).map((v) => (
                    <option key={v.id} value={v.id}>
                      Minecraft {v.id}
                    </option>
                  ))}
                </select>
                <span className="home__muted">Fabric {inst?.loaderVersion}</span>
              </dd>
            </div>
            <div>
              <dt>Profile</dt>
              <dd>
                {profileName}
                <button className="btn btn--quiet" onClick={() => onNavigate("profiles")}>
                  Change
                </button>
              </dd>
            </div>
          </dl>

          <div className="launch">
            {phase === "running" ? (
              <>
                <button className="launch__button" disabled>
                  Playing {version}
                </button>
                <button className="btn" onClick={() => api.stop().catch((e) => notify("error", "Couldn't stop the game", [errorText(e)]))}>
                  Stop game
                </button>
              </>
            ) : (
              <button className="launch__button" disabled={!active || phase === "launching"} onClick={launch}>
                {phase === "launching" ? "Starting…" : `Launch ${version}`}
              </button>
            )}
          </div>

          {phase === "launching" && progress && (
            <div className="launch__progress" role="status">
              <div className="launch__track">
                <div className={`launch__fill ${pct === null ? "launch__fill--busy" : ""}`} style={{ width: pct === null ? "30%" : `${pct}%` }} />
              </div>
              <span>{progress.label}</span>
            </div>
          )}
          {!active && <p className="home__muted">Sign in with the Microsoft account that owns Minecraft: Java Edition.</p>}

          <div className="home__log">
            <button className="btn btn--quiet" onClick={() => setShowLog(!showLog)} aria-expanded={showLog} disabled={log.length === 0}>
              {showLog ? "Hide game log" : "Show game log"}
            </button>
            {showLog && <GameLog lines={log} />}
          </div>
        </div>
      </div>
    </>
  );
}

function AccountHeader({ view, onChanged, onAdd }: { view: AccountsView; onChanged: () => void; onAdd: () => void }) {
  const notify = useNotify();
  const active = view.accounts.find((a) => a.active)!;
  const others = view.accounts.filter((a) => !a.active);

  return (
    <div className="home__account">
      <h1 className="home__name">{active.name}</h1>
      <p className="home__muted">
        {active.kind === "test" ? "Test account (development builds only, offline)" : "Microsoft account"}
      </p>
      <div className="home__account-actions">
        {others.length > 0 && (
          <select
            className="field"
            value=""
            aria-label="Switch account"
            onChange={(e) => api.selectAccount(e.target.value).then(onChanged)}
          >
            <option value="" disabled>
              Switch account
            </option>
            {others.map((a) => (
              <option key={a.uuid} value={a.uuid}>
                {a.name}
              </option>
            ))}
          </select>
        )}
        {view.clientIdSet && (
          <button className="btn btn--quiet" onClick={onAdd}>
            Add account
          </button>
        )}
        <button
          className="btn btn--quiet"
          onClick={() =>
            api
              .signOut(active.uuid)
              .then(onChanged)
              .catch((e) => notify("error", "Couldn't sign out", [errorText(e)]))
          }
        >
          Sign out
        </button>
      </div>
    </div>
  );
}

interface SignInProps {
  view: AccountsView;
  code: DeviceCode | null;
  signingIn: string | null;
  onBegin: () => void;
  onSaved: () => void;
}

function SignIn({ view, code, signingIn, onBegin, onSaved }: SignInProps) {
  const notify = useNotify();
  const [clientId, setClientId] = useState("");
  const [testName, setTestName] = useState("");

  if (code) {
    return (
      <div className="signin">
        <p className="signin__step">Go to {code.verificationUri.replace(/^https?:\/\//, "")} and enter this code:</p>
        <p className="signin__code">{code.userCode}</p>
        <div className="signin__actions">
          <button className="btn btn--primary" onClick={() => api.openSignInPage()}>
            Open sign-in page
          </button>
          <button className="btn" onClick={() => navigator.clipboard.writeText(code.userCode).then(() => notify("ok", "Code copied"))}>
            Copy code
          </button>
          <button className="btn btn--quiet" onClick={() => api.cancelSignIn()}>
            Cancel
          </button>
        </div>
        <p className="home__muted">{signingIn ?? "Waiting for you to finish signing in…"}</p>
      </div>
    );
  }

  return (
    <div className="signin">
      {view.clientIdSet ? (
        <button className="launch__button launch__button--secondary" onClick={onBegin}>
          Sign in with Microsoft
        </button>
      ) : (
        <form
          className="signin__form"
          onSubmit={(e) => {
            e.preventDefault();
            api
              .setClientId(clientId)
              .then(() => {
                notify("ok", "Saved your Azure client ID");
                onSaved();
              })
              .catch((err) => notify("error", "Couldn't save the client ID", [errorText(err)]));
          }}
        >
          <label className="signin__step" htmlFor="client-id">
            Microsoft sign-in needs your Azure app's Application (client) ID. It's on the app's Overview page in portal.azure.com.
          </label>
          <div className="searchbar">
            <input
              id="client-id"
              className="field field--grow"
              placeholder="00000000-0000-0000-0000-000000000000"
              value={clientId}
              onChange={(e) => setClientId(e.target.value)}
              spellCheck={false}
            />
            <button className="btn btn--primary" type="submit" disabled={clientId.trim().length === 0}>
              Save
            </button>
          </div>
        </form>
      )}

      {view.testAccountsAllowed && (
        <form
          className="signin__form signin__form--dev"
          onSubmit={(e) => {
            e.preventDefault();
            api
              .addTestAccount(testName)
              .then(onSaved)
              .catch((err) => notify("error", "Couldn't add the test account", [errorText(err)]));
          }}
        >
          <label className="signin__step" htmlFor="test-name">
            While your Azure app waits for approval, a test account can launch singleplayer and offline-mode servers. It only exists in development builds.
          </label>
          <div className="searchbar">
            <input id="test-name" className="field field--grow" placeholder="Test name" value={testName} onChange={(e) => setTestName(e.target.value)} maxLength={16} />
            <button className="btn" type="submit" disabled={testName.length < 3}>
              Use test account
            </button>
          </div>
        </form>
      )}
    </div>
  );
}

function GameLog({ lines }: { lines: string[] }) {
  const ref = useRef<HTMLPreElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);
  return (
    <pre className="log" ref={ref} tabIndex={0} aria-label="Game log">
      {lines.slice(-400).join("\n")}
    </pre>
  );
}
