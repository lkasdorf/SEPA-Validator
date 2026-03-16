# Pruefung neuer Inbox-Dateien am 2026-03-16 22:55:59

Gepruefte Dateien:

- `to_check/inbox/UP_Payment_Media_T.xml`
- `to_check/inbox/UP_Payment_Media_F.xml`

Beide Dateien wurden nach der Pruefung nach `to_check/invalid/` verschoben.

## UP_Payment_Media_T.xml

Status:

- Nicht valide gegen `pain.001.001.09.xsd`

Korrekturliste:

- Namespace im Root-Element korrigieren:
  `urn:iso:std:iso:20022:tech:xs:d:pain.001.001.09`
  zu
  `urn:iso:std:iso:20022:tech:xsd:pain.001.001.09`
- In `Cdtr/PstlAdr` ist die Elementreihenfolge ungueltig. `Ctry` steht zu frueh.
- Die Postadresse des Creditors muss in der vom Schema erwarteten Reihenfolge aufgebaut sein.

## UP_Payment_Media_F.xml

Status:

- Nicht valide gegen `pain.001.001.09.xsd`

Korrekturliste:

- Namespace im Root-Element korrigieren:
  `urn:iso:std:iso:20022:tech:xs:d:pain.001.001.09`
  zu
  `urn:iso:std:iso:20022:tech:xsd:pain.001.001.09`
- `Dbtr/CtryOfRes` ist an dieser Stelle im pain.001.001.09-Schema nicht zulaessig.
- In `CdtTrfTxInf/PmtTpInf` ist `InstrPrty` allein ungueltig; das Schema erwartet dort `SvcLvl`.
- `ChrgBr` hat den Wert `SHAR`; fuer SEPA pain.001.001.09 ist `SLEV` erforderlich.
- `CdtrAgt/FinInstnId` verwendet `ClrSysMmbId`; an dieser Stelle wird `BICFI` erwartet.
- `CdtrAcct/Id` verwendet `Othr/BBAN`; fuer dieses Schema wird hier `IBAN` erwartet.

## Report

- Validierungslauf: `analysis/validation_20260316_225421.csv`
- Aktueller Report: `analysis/validation_latest.csv`
