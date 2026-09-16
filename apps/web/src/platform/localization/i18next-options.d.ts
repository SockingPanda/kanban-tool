import 'i18next';
/** This project owns its generated key/parameter contract. Keep the string API explicit across i18next 26. */
declare module 'i18next' {
  interface CustomTypeOptions { enableSelector: false }
}
