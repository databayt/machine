// tasks — every string the work-order submit shows, ar + en (kun's inline-copy way, kept in one place).
export interface Copy {
  en: string
  ar: string
}

export const copy = {
  submit: { en: "Submit", ar: "سلّم العمل" },
  submitAsk: {
    en: "Write one line about the work in the box, then press Submit.",
    ar: "اكتب سطراً عن العمل في الصندوق ثم اضغط سلّم العمل.",
  },
  submitPick: { en: "Choose the files to submit", ar: "اختر الملفات لتسليمها" },
  submitDone: { en: "Submitted. It is waiting for review.", ar: "تم التسليم. بانتظار المراجعة." },
} satisfies Record<string, Copy>
