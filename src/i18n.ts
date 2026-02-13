import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import fr from './locales/fr.json'
import de from './locales/de.json'

const resources = {
  en: { translation: en },
  fr: { translation: fr },
  de: { translation: de }
} as const

const detectLanguage = () => {
  const lang = navigator.language?.toLowerCase() ?? 'en'
  if (lang.startsWith('fr')) return 'fr'
  if (lang.startsWith('de')) return 'de'
  return 'en'
}

i18n.use(initReactI18next).init({
  resources,
  lng: detectLanguage(),
  fallbackLng: 'en',
  interpolation: { escapeValue: false }
})

export default i18n
