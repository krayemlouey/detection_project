-- =====================================================
-- SYSTÈME DE DÉTECTION D'OBJETS IoT - BASE DE DONNÉES
-- Script d'initialisation SQLite3
-- =====================================================

-- Supprimer les tables existantes (optionnel, pour réinitialisation)
-- DROP TABLE IF EXISTS daily_stats;
-- DROP TABLE IF EXISTS detections;

-- ===== TABLE 1: DÉTECTIONS INDIVIDUELLES =====
CREATE TABLE IF NOT EXISTS detections (
    -- Clé primaire auto-incrémentée
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    
    -- G_ID : Identifiant unique pour chaque type de carte
    -- Format: COULEUR_TYPE_TIMESTAMP (ex: RED_MICROCHIP_1699123456)
    g_id TEXT NOT NULL UNIQUE,
    
    -- Compteur de référence (G_ID * ref)
    -- S'incrémente à chaque détection du même G_ID
    ref_count INTEGER NOT NULL DEFAULT 1,
    
    -- Type d'objet détecté
    type TEXT NOT NULL,
    
    -- Couleur détectée (red, green, blue)
    color TEXT NOT NULL,
    
    -- Date et heure de la détection
    datetime TEXT NOT NULL,
    
    -- Index sur G_ID pour les requêtes rapides
    CONSTRAINT unique_g_id UNIQUE (g_id)
);

-- ===== TABLE 2: STATISTIQUES JOURNALIÈRES =====
CREATE TABLE IF NOT EXISTS daily_stats (
    -- Clé primaire auto-incrémentée
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    
    -- G_ID de référence
    g_id TEXT NOT NULL,
    
    -- Type d'objet
    type TEXT NOT NULL,
    
    -- Cadence : nombre de détections pour ce G_ID ce jour
    cadence INTEGER NOT NULL DEFAULT 0,
    
    -- Jour au format YYYY-MM-DD
    day TEXT NOT NULL,
    
    -- Contrainte d'unicité : un seul enregistrement par G_ID et par jour
    CONSTRAINT unique_g_id_day UNIQUE (g_id, day)
);

-- ===== INDEX POUR OPTIMISATION =====

-- Index sur les détections par date
CREATE INDEX IF NOT EXISTS idx_detections_datetime 
ON detections(datetime);

-- Index sur les détections par type
CREATE INDEX IF NOT EXISTS idx_detections_type 
ON detections(type);

-- Index sur les détections par couleur
CREATE INDEX IF NOT EXISTS idx_detections_color 
ON detections(color);

-- Index sur les statistiques journalières par jour
CREATE INDEX IF NOT EXISTS idx_daily_stats_day 
ON daily_stats(day);

-- Index sur les statistiques journalières par type
CREATE INDEX IF NOT EXISTS idx_daily_stats_type 
ON daily_stats(type);

-- ===== DONNÉES D'EXEMPLE (OPTIONNEL) =====
-- Uncomment pour insérer des données de test

/*
INSERT OR IGNORE INTO detections (g_id, ref_count, type, color, datetime) VALUES
('RED_MICROCHIP_001', 1, 'Carte microchip', 'red', '2024-01-15 10:30:00'),
('GREEN_CUSTOM_001', 1, 'Carte personnalisée', 'green', '2024-01-15 10:35:00'),
('BLUE_STM32_001', 1, 'STM32', 'blue', '2024-01-15 10:40:00'),
('RED_MICROCHIP_001', 2, 'Carte microchip', 'red', '2024-01-15 10:45:00'),
('GREEN_CUSTOM_002', 1, 'Carte personnalisée', 'green', '2024-01-15 11:00:00');

INSERT OR IGNORE INTO daily_stats (g_id, type, cadence, day) VALUES
('RED_MICROCHIP_001', 'Carte microchip', 2, '2024-01-15'),
('GREEN_CUSTOM_001', 'Carte personnalisée', 1, '2024-01-15'),
('GREEN_CUSTOM_002', 'Carte personnalisée', 1, '2024-01-15'),
('BLUE_STM32_001', 'STM32', 1, '2024-01-15');
*/

-- ===== VUES UTILES =====

-- Vue pour les statistiques quotidiennes par type
CREATE VIEW IF NOT EXISTS daily_type_stats AS
SELECT 
    day,
    type,
    SUM(cadence) as total_detections,
    COUNT(DISTINCT g_id) as unique_objects
FROM daily_stats 
GROUP BY day, type
ORDER BY day DESC, total_detections DESC;

-- Vue pour les détections récentes avec comptage
CREATE VIEW IF NOT EXISTS recent_detections AS
SELECT 
    d.g_id,
    d.type,
    d.color,
    d.ref_count,
    d.datetime,
    -- Rang de la détection (plus récent = 1)
    ROW_NUMBER() OVER (ORDER BY d.datetime DESC) as rank
FROM detections d
ORDER BY d.datetime DESC
LIMIT 50;

-- Vue pour les statistiques globales
CREATE VIEW IF NOT EXISTS global_stats AS
SELECT 
    type,
    color,
    COUNT(DISTINCT g_id) as unique_objects,
    SUM(ref_count) as total_detections,
    MIN(datetime) as first_detection,
    MAX(datetime) as last_detection
FROM detections 
GROUP BY type, color;

-- ===== TRIGGERS POUR MAINTIEN DE LA COHÉRENCE =====

-- Trigger pour mettre à jour automatiquement les stats journalières
CREATE TRIGGER IF NOT EXISTS update_daily_stats_on_insert
AFTER INSERT ON detections
BEGIN
    -- Insérer ou mettre à jour les statistiques du jour
    INSERT OR REPLACE INTO daily_stats (g_id, type, cadence, day)
    VALUES (
        NEW.g_id,
        NEW.type,
        COALESCE(
            (SELECT cadence FROM daily_stats 
             WHERE g_id = NEW.g_id AND day = DATE(NEW.datetime)), 0
        ) + 1,
        DATE(NEW.datetime)
    );
END;

-- Trigger pour nettoyer les stats journalières lors de suppression
CREATE TRIGGER IF NOT EXISTS cleanup_daily_stats_on_delete
AFTER DELETE ON detections
BEGIN
    -- Décrémenter ou supprimer les statistiques journalières
    UPDATE daily_stats 
    SET cadence = cadence - OLD.ref_count
    WHERE g_id = OLD.g_id AND day = DATE(OLD.datetime);
    
    -- Supprimer l'entrée si cadence devient 0
    DELETE FROM daily_stats 
    WHERE g_id = OLD.g_id AND day = DATE(OLD.datetime) AND cadence <= 0;
END;

-- ===== PROCÉDURES DE MAINTENANCE =====

-- Note: SQLite ne supporte pas les procédures stockées nativement
-- Ces requêtes peuvent être exécutées manuellement ou via l'application

-- Nettoyer les données anciennes (plus de 90 jours)
/*
DELETE FROM daily_stats 
WHERE day < DATE('now', '-90 days');

DELETE FROM detections 
WHERE datetime < DATETIME('now', '-90 days');
*/

-- Recalculer les statistiques journalières (en cas d'incohérence)
/*
DELETE FROM daily_stats;

INSERT INTO daily_stats (g_id, type, cadence, day)
SELECT 
    g_id,
    type,
    COUNT(*) as cadence,
    DATE(datetime) as day
FROM detections
GROUP BY g_id, type, DATE(datetime);
*/

-- Réinitialiser les compteurs de référence
/*
UPDATE detections 
SET ref_count = (
    SELECT COUNT(*) 
    FROM detections d2 
    WHERE d2.g_id = detections.g_id 
    AND d2.datetime <= detections.datetime
);
*/

-- ===== REQUÊTES UTILES POUR L'APPLICATION =====

-- Obtenir les détections du jour actuel
/*
SELECT * FROM detections 
WHERE DATE(datetime) = DATE('now')
ORDER BY datetime DESC;
*/

-- Statistiques par type pour la semaine courante
/*
SELECT 
    type,
    SUM(cadence) as weekly_total
FROM daily_stats 
WHERE day >= DATE('now', '-7 days')
GROUP BY type;
*/

-- Top 10 des G_ID les plus détectés
/*
SELECT 
    g_id,
    type,
    ref_count,
    datetime as last_seen
FROM detections 
ORDER BY ref_count DESC 
LIMIT 10;
*/

-- Évolution journalière des détections (7 derniers jours)
/*
SELECT 
    day,
    SUM(CASE WHEN type = 'Carte microchip' THEN cadence ELSE 0 END) as microchip,
    SUM(CASE WHEN type = 'Carte personnalisée' THEN cadence ELSE 0 END) as custom,
    SUM(CASE WHEN type = 'STM32' THEN cadence ELSE 0 END) as stm32,
    SUM(cadence) as total
FROM daily_stats 
WHERE day >= DATE('now', '-7 days')
GROUP BY day
ORDER BY day;
*/

-- ===== INFORMATIONS SYSTÈME =====

-- Version SQLite et configuration
PRAGMA compile_options;
PRAGMA journal_mode = WAL; -- Write-Ahead Logging pour de meilleures performances
PRAGMA synchronous = NORMAL; -- Équilibre entre performance et sécurité
PRAGMA cache_size = 10000; -- Cache plus large pour les requêtes

-- Activer les clés étrangères (si nécessaire plus tard)
PRAGMA foreign_keys = ON;

-- Optimisation pour les performances
PRAGMA temp_store = MEMORY; -- Utiliser la RAM pour les tables temporaires

-- ===== COMMENTAIRES DE DOCUMENTATION =====

/*
STRUCTURE DE LA BASE DE DONNÉES:

1. TABLE DETECTIONS:
   - Stocke chaque détection individuelle
   - G_ID unique permet d'identifier le type de carte
   - ref_count s'incrémente pour chaque nouvelle détection du même G_ID
   - Index optimisés pour les requêtes par date, type et couleur

2. TABLE DAILY_STATS:
   - Agrège les détections par jour et par G_ID
   - Permet des requêtes rapides sur les statistiques journalières
   - Mise à jour automatique via triggers

3. VUES:
   - Facilitent l'accès aux données agrégées
   - Pas de stockage supplémentaire, juste des requêtes précompilées

4. TRIGGERS:
   - Maintiennent automatiquement la cohérence entre les tables
   - Évitent la duplication de logique côté application

UTILISATION:
- L'application Rust utilise principalement les tables de base
- Les vues servent pour les rapports et statistiques
- Les triggers assurent l'intégrité automatiquement
*/