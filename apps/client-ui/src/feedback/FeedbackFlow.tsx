import {
  type ChangeEvent,
  type FormEvent,
  useEffect,
  useRef,
  useState,
} from 'react';
import { DialogFrame } from '../components/DialogFrame';
import {
  clientFailureCode,
  type ClientAdapter,
  type FeedbackStatus,
} from '../domain';
import { copyFor } from '../i18n';

const screenshotLimit = 1_250_000;

type AppCopy = ReturnType<typeof copyFor>;

interface FeedbackFlowProps {
  open: boolean;
  adapter: ClientAdapter;
  copy: AppCopy;
  screenContext: string;
  onClose: () => void;
}

function readImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    const url = URL.createObjectURL(file);
    image.onload = () => {
      URL.revokeObjectURL(url);
      resolve(image);
    };
    image.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('image-decode'));
    };
    image.src = url;
  });
}

async function prepareScreenshot(file: File): Promise<string> {
  if (!['image/png', 'image/jpeg'].includes(file.type)) {
    throw new Error('image-type');
  }
  const image = await readImage(file);
  const scale = Math.min(
    1,
    1280 / Math.max(image.naturalWidth, 1),
    1280 / Math.max(image.naturalHeight, 1),
  );
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('image-canvas');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  for (const quality of [0.78, 0.64, 0.5]) {
    const encoded = canvas.toDataURL('image/jpeg', quality);
    if (encoded.length <= screenshotLimit) return encoded;
  }
  throw new Error('image-size');
}

export function FeedbackFlow({
  open,
  adapter,
  copy,
  screenContext,
  onClose,
}: FeedbackFlowProps) {
  const [whatHappened, setWhatHappened] = useState('');
  const [expected, setExpected] = useState('');
  const [screenshot, setScreenshot] = useState('');
  const [screenshotName, setScreenshotName] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [sending, setSending] = useState(false);
  const [notice, setNotice] = useState('');
  const [receipt, setReceipt] = useState<FeedbackStatus | null>(null);
  const receiptRevision = useRef(0);

  useEffect(() => {
    if (!open) return undefined;

    let active = true;
    const requestedRevision = receiptRevision.current;
    setNotice('');
    void adapter.feedbackStatus().then((status) => {
      if (active && requestedRevision === receiptRevision.current) {
        setReceipt(status);
      }
    }).catch(() => {
      // A missing receipt or offline status check must not block a new report.
    });

    return () => {
      active = false;
    };
  }, [adapter, open]);

  async function chooseScreenshot(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    setNotice(copy.preparingScreenshot);
    try {
      setScreenshot(await prepareScreenshot(file));
      setScreenshotName(file.name);
      setNotice(copy.screenshotReady);
    } catch {
      setScreenshot('');
      setScreenshotName('');
      setNotice(copy.screenshotInvalid);
    }
  }

  async function sendFeedback(event: FormEvent) {
    event.preventDefault();
    if (!whatHappened.trim() || !expected.trim() || !confirmed || sending) {
      setNotice(copy.feedbackIncomplete);
      return;
    }
    setSending(true);
    setNotice(copy.feedbackSending);
    receiptRevision.current += 1;
    try {
      const nextReceipt = await adapter.submitFeedback({
        whatHappened,
        expected,
        screen: screenContext,
        screenshot,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
        confirmed,
      });
      setReceipt(nextReceipt);
      setWhatHappened('');
      setExpected('');
      setScreenshot('');
      setScreenshotName('');
      setConfirmed(false);
      setNotice(copy.feedbackDelivered);
    } catch (reason) {
      setNotice(copy.errorMessage(clientFailureCode(reason)));
    } finally {
      setSending(false);
    }
  }

  if (!open) return null;

  return (
    <DialogFrame title={copy.feedbackTitle} closeLabel={copy.close} onClose={onClose}>
      <form className="dialog-form feedback-form" onSubmit={sendFeedback}>
        <div className="dialog-scroll">
          <p>{copy.feedbackPrivacy}</p>
          <label>
            <span>{copy.whatHappened}</span>
            <textarea
              rows={6}
              value={whatHappened}
              placeholder={copy.whatHappenedHint}
              onChange={(event) => setWhatHappened(event.target.value)}
            />
          </label>
          <label>
            <span>{copy.whatExpected}</span>
            <textarea
              rows={5}
              value={expected}
              placeholder={copy.whatExpectedHint}
              onChange={(event) => setExpected(event.target.value)}
            />
          </label>
          <div className="screenshot-control">
            <span>{copy.optionalScreenshot}</span>
            {screenshotName ? (
              <div>
                <strong>{screenshotName}</strong>
                <button
                  type="button"
                  onClick={() => {
                    setScreenshot('');
                    setScreenshotName('');
                  }}
                >
                  {copy.remove}
                </button>
              </div>
            ) : (
              <label className="secondary-action">
                {copy.chooseScreenshot}
                <input
                  type="file"
                  accept="image/png,image/jpeg"
                  onChange={chooseScreenshot}
                />
              </label>
            )}
            <small>{copy.screenshotPrivacy}</small>
          </div>
          {receipt && (
            <div className="feedback-receipt">
              <strong>{copy.feedbackReference} · {receipt.publicId}</strong>
              <span>{copy.feedbackState(receipt.status)}</span>
              {receipt.adminReply && <p>{copy.feedbackReply}: {receipt.adminReply}</p>}
            </div>
          )}
          {notice && <p className="dialog-notice" role="status">{notice}</p>}
        </div>
        <footer className="feedback-footer">
          <label className="confirmation">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(event) => setConfirmed(event.target.checked)}
            />
            <span>{copy.feedbackConfirmation}</span>
          </label>
          <div>
            <button className="secondary-action" type="button" onClick={onClose}>
              {copy.cancel}
            </button>
            <button
              className="primary-action"
              type="submit"
              disabled={sending || !confirmed || !whatHappened.trim() || !expected.trim()}
            >
              {sending ? copy.feedbackSending : copy.sendFeedback}
            </button>
          </div>
        </footer>
      </form>
    </DialogFrame>
  );
}
