# 12 — Effets et temps réel

Décision : [ADR 0018](../adr/0018-profil-temps-reel.md). Ce chapitre décrit ce que le
compilateur vérifie. Ce qui n'est pas encore implémenté est signalé « à venir » ; le
compilateur le refuse (E0111) plutôt que de l'accepter sans le vérifier.

## 12.1 Effets : `with noalloc, nonblocking`

```nexul
fn i64 filtrer(Filtre f!, i64 x) with noalloc, nonblocking {
    f.somme += x - f.fenetre[f.pos]
    f.fenetre[f.pos] = x
    f.pos = (f.pos + 1) % 4
    return f.somme / 4
}
```

- La clause suit les paramètres, avant le corps. Effets : `noalloc` et `nonblocking`, une
  fois chacun (E0604). Une autre étiquette est refusée (E0111).
- **`noalloc`** : la fonction ne peut ni allouer ni libérer de mémoire du tas, directement
  ou par une fonction qu'elle appelle (E0601). Cela comprend `Vec.push` et les chaînes qui
  grandissent, l'interpolation de chaînes, la conversion vers `any`, les parties
  `indirect` et la **destruction** d'une valeur qui possède du tas (`String`, `Vec`…).
- **`nonblocking`** : la fonction ne peut pas attendre (E0602) : console (`println`),
  fichiers, semi-hébergement, verrous et canaux de `std.tache`, `join`, `sleep`, appels
  système.
- Dans les deux cas, la fonction ne peut faire ni appel **indirect** (valeur de fonction,
  appel à travers `any`) ni **récursion** (E0603).
- La vérification suit tous les appels, dans les instances génériques réelles
  (`premier[i32]`) et les destructeurs. Les fonctions appelées n'ont pas besoin d'être
  annotées. L'erreur montre la chaîne d'appels.
- **Arrêts** : une panique (débordement, indice hors bornes, `panic`, `assert`) et un appel
  à une fonction qui ne rend jamais la main (`Never`) ne sont pas des effets : le
  programme s'arrête.
- **Fonctions C** : un appel à une fonction `extern "C"` alloue et bloque, sauf si sa
  déclaration porte la clause. C'est alors une promesse du programmeur, que le compilateur
  ne vérifie pas :

  ```nexul
  extern "C" fn i32 abs(i32 x) with noalloc, nonblocking
  ```

- La bibliothèque standard marque ses racines par `@allocates` (allocateur) et `@blocks`
  (appel système, semi-hébergement, attente). Ces deux attributs lui sont réservés (E0231).
- Les effets sont vérifiés sur toutes les cibles ; ils ne promettent rien sur la durée.

## 12.2 Tâches périodiques, ressources, interruptions (à venir)

`every … within …`, `@resource`, `@interrupt`, `@priority`, `rt.run()` : ADR 0018 §2 à §5,
prévus en 0.7.0 sur `thumbv7em-none`. Les durées s'écrivent avec `std.rt.ms`, `us` et `s`
(en nanosecondes).

## 12.3 Pile bornée (à venir)

ADR 0018 §6 : borne calculée sur le code machine, réserve `@stack`, garde MPU (E0607).
