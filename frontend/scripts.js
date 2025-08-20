/**
 * SYSTÈME DE DÉTECTION D'OBJETS IoT - FRONTEND
 * Script principal pour la gestion de l'interface utilisateur
 */

// ===== VARIABLES GLOBALES =====
let cameraStream = null;
let detectionInterval = null;
let isDetectionActive = false;
let detectionCounts = {
    red: 0,
    green: 0, 
    blue: 0
};
let lastDetectionTime = 0;
let fpsCounter = 0;
let fpsInterval = null;

// Configuration
const CONFIG = {
    API_BASE_URL: '/api',
    DETECTION_INTERVAL: 1000, // 1000ms comme demandé
    FPS_UPDATE_INTERVAL: 1000,
    MAX_CANVAS_WIDTH: 640,
    MAX_CANVAS_HEIGHT: 480
};

// ===== INITIALISATION =====
document.addEventListener('DOMContentLoaded', function() {
    checkAuthentication();
    initializeDashboard();
    setupEventListeners();
    loadRecentDetections();
    startFPSCounter();
});

/**
 * Vérifie l'authentification de l'utilisateur
 */
function checkAuthentication() {
    const sessionToken = localStorage.getItem('sessionToken');
    if (!sessionToken) {
        window.location.href = 'login.html';
        return;
    }
    
    console.log('✅ Utilisateur authentifié');
}

/**
 * Initialise le dashboard principal
 */
function initializeDashboard() {
    console.log('🚀 Initialisation du dashboard...');
    
    // Vérifier le support de la caméra
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
        showNotification('Votre navigateur ne supporte pas l\'accès à la caméra', 'error');
        return;
    }
    
    // Initialiser les contrôles
    updateSensitivityDisplay();
    updateDetectionInterval();
    
    console.log('✅ Dashboard initialisé');
}

/**
 * Configure tous les écouteurs d'événements
 */
function setupEventListeners() {
    // Contrôles caméra
    document.getElementById('startCameraBtn').addEventListener('click', startCamera);
    document.getElementById('stopCameraBtn').addEventListener('click', stopCamera);
    document.getElementById('captureBtn').addEventListener('click', captureFrame);
    
    // Contrôles généraux
    document.getElementById('historyBtn').addEventListener('click', () => {
        window.location.href = 'history.html';
    });
    
    document.getElementById('resetCountersBtn').addEventListener('click', resetCounters);
    document.getElementById('refreshPreviewBtn').addEventListener('click', loadRecentDetections);
    
    // Contrôles de configuration
    document.getElementById('detectionSensitivity').addEventListener('input', updateSensitivityDisplay);
    document.getElementById('detectionInterval').addEventListener('change', updateDetectionInterval);
    
    // Modal d'ajout d'objet
    document.getElementById('addObjectBtn').addEventListener('click', () => {
        document.getElementById('addObjectModal').style.display = 'block';
    });
    
    document.getElementById('addObjectForm').addEventListener('submit', handleAddObject);
    
    // Fermeture des modals
    document.querySelectorAll('.close').forEach(closeBtn => {
        closeBtn.addEventListener('click', function() {
            this.closest('.modal').style.display = 'none';
        });
    });
    
    // Fermer modal en cliquant à l'extérieur
    window.addEventListener('click', function(event) {
        if (event.target.classList.contains('modal')) {
            event.target.style.display = 'none';
        }
    });
}

// ===== GESTION CAMÉRA =====

/**
 * Démarre la caméra et la détection
 */
async function startCamera() {
    try {
        console.log('📹 Démarrage de la caméra...');
        
        // Demander l'accès à la caméra
        cameraStream = await navigator.mediaDevices.getUserMedia({
            video: {
                width: { ideal: CONFIG.MAX_CANVAS_WIDTH },
                height: { ideal: CONFIG.MAX_CANVAS_HEIGHT },
                frameRate: { ideal: 30 }
            },
            audio: false
        });
        
        // Configurer l'affichage vidéo
        const videoElement = document.getElementById('cameraFeed');
        videoElement.srcObject = cameraStream;
        
        // Attendre que la vidéo soit prête
        await new Promise((resolve) => {
            videoElement.addEventListener('loadedmetadata', resolve, { once: true });
        });
        
        // Configurer le canvas overlay
        const canvas = document.getElementById('detectionOverlay');
        canvas.width = videoElement.videoWidth || CONFIG.MAX_CANVAS_WIDTH;
        canvas.height = videoElement.videoHeight || CONFIG.MAX_CANVAS_HEIGHT;
        canvas.style.width = videoElement.offsetWidth + 'px';
        canvas.style.height = videoElement.offsetHeight + 'px';
        
        // Démarrer la détection
        startDetection();
        
        // Mettre à jour l'interface
        document.getElementById('startCameraBtn').disabled = true;
        document.getElementById('stopCameraBtn').disabled = false;
        document.getElementById('captureBtn').disabled = false;
        
        updateStatusIndicator(true);
        updateDetectionStatus('Détection active');
        
        showNotification('Caméra démarrée avec succès', 'success');
        console.log('✅ Caméra démarrée');
        
    } catch (error) {
        console.error('❌ Erreur caméra:', error);
        
        let message = 'Impossible d\'accéder à la caméra';
        if (error.name === 'NotAllowedError') {
            message = 'Accès à la caméra refusé. Veuillez autoriser l\'accès.';
        } else if (error.name === 'NotFoundError') {
            message = 'Aucune caméra trouvée. Vérifiez votre connexion.';
        }
        
        showNotification(message, 'error');
        updateDetectionStatus('Erreur caméra');
    }
}

/**
 * Arrête la caméra et la détection
 */
function stopCamera() {
    console.log('⏹️ Arrêt de la caméra...');
    
    // Arrêter la détection
    stopDetection();
    
    // Arrêter le flux caméra
    if (cameraStream) {
        cameraStream.getTracks().forEach(track => {
            track.stop();
        });
        cameraStream = null;
    }
    
    // Nettoyer l'affichage vidéo
    const videoElement = document.getElementById('cameraFeed');
    videoElement.srcObject = null;
    
    // Nettoyer le canvas
    const canvas = document.getElementById('detectionOverlay');
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    
    // Mettre à jour l'interface
    document.getElementById('startCameraBtn').disabled = false;
    document.getElementById('stopCameraBtn').disabled = true;
    document.getElementById('captureBtn').disabled = true;
    
    updateStatusIndicator(false);
    updateDetectionStatus('Arrêtée');
    
    showNotification('Caméra arrêtée', 'info');
    console.log('✅ Caméra arrêtée');
}

/**
 * Démarre la boucle de détection
 */
function startDetection() {
    if (isDetectionActive) return;
    
    isDetectionActive = true;
    console.log('🎯 Démarrage de la détection...');
    
    detectionInterval = setInterval(() => {
        if (cameraStream && isDetectionActive) {
            processFrame();
        }
    }, CONFIG.DETECTION_INTERVAL);
    
    console.log(`✅ Détection démarrée (intervalle: ${CONFIG.DETECTION_INTERVAL}ms)`);
}

/**
 * Arrête la boucle de détection
 */
function stopDetection() {
    if (!isDetectionActive) return;
    
    isDetectionActive = false;
    
    if (detectionInterval) {
        clearInterval(detectionInterval);
        detectionInterval = null;
    }
    
    console.log('⏹️ Détection arrêtée');
}

// ===== TRAITEMENT DES FRAMES =====

/**
 * Traite une frame pour la détection de couleurs
 */
function processFrame() {
    try {
        const video = document.getElementById('cameraFeed');
        const canvas = document.getElementById('detectionOverlay');
        
        if (!video.videoWidth || !video.videoHeight) {
            return;
        }
        
        // Créer un canvas temporaire pour capturer la frame
        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = video.videoWidth;
        tempCanvas.height = video.videoHeight;
        const tempCtx = tempCanvas.getContext('2d');
        
        // Capturer la frame actuelle
        tempCtx.drawImage(video, 0, 0);
        const imageData = tempCtx.getImageData(0, 0, tempCanvas.width, tempCanvas.height);
        
        // Analyser les couleurs
        const detections = analyzeColors(imageData, tempCanvas.width, tempCanvas.height);
        
        // Dessiner les détections sur l'overlay
        drawDetections(canvas, detections);
        
        // Envoyer les détections à l'API si nécessaire
        processDetections(detections);
        
        // Mettre à jour les compteurs FPS
        fpsCounter++;
        
    } catch (error) {
        console.error('Erreur traitement frame:', error);
    }
}

/**
 * Analyse les couleurs dans l'image
 */
function analyzeColors(imageData, width, height) {
    const data = imageData.data;
    const detections = [];
    const sensitivity = parseInt(document.getElementById('detectionSensitivity').value);
    const threshold = 255 - (sensitivity * 20); // Plus la sensibilité est haute, plus le seuil est bas
    
    // Définition des seuils de couleur adaptés à la sensibilité
    const colorThresholds = {
        red: {
            min: { r: Math.max(100, threshold), g: 0, b: 0 },
            max: { r: 255, g: 80, b: 80 },
            label: 'Carte microchip',
            color: 'red'
        },
        green: {
            min: { r: 0, g: Math.max(100, threshold), b: 0 },
            max: { r: 80, g: 255, b: 80 },
            label: 'Carte personnalisée',
            color: 'green'
        },
        blue: {
            min: { r: 0, g: 0, b: Math.max(100, threshold) },
            max: { r: 80, g: 80, b: 255 },
            label: 'STM32',
            color: 'blue'
        }
    };
    
    // Analyser par zones pour améliorer les performances
    const blockSize = 20;
    for (let y = 0; y < height; y += blockSize) {
        for (let x = 0; x < width; x += blockSize) {
            const index = (y * width + x) * 4;
            
            if (index < data.length) {
                const r = data[index];
                const g = data[index + 1];
                const b = data[index + 2];
                
                // Vérifier chaque couleur
                for (const [colorName, threshold] of Object.entries(colorThresholds)) {
                    if (isColorMatch(r, g, b, threshold)) {
                        detections.push({
                            x: x + blockSize / 2,
                            y: y + blockSize / 2,
                            width: blockSize * 2,
                            height: blockSize * 2,
                            color: colorName,
                            label: threshold.label,
                            confidence: calculateConfidence(r, g, b, threshold)
                        });
                    }
                }
            }
        }
    }
    
    // Grouper les détections proches
    return groupNearbyDetections(detections);
}

/**
 * Vérifie si une couleur RGB correspond à un seuil
 */
function isColorMatch(r, g, b, threshold) {
    return r >= threshold.min.r && r <= threshold.max.r &&
           g >= threshold.min.g && g <= threshold.max.g &&
           b >= threshold.min.b && b <= threshold.max.b;
}

/**
 * Calcule la confiance de détection
 */
function calculateConfidence(r, g, b, threshold) {
    const centerR = (threshold.min.r + threshold.max.r) / 2;
    const centerG = (threshold.min.g + threshold.max.g) / 2;
    const centerB = (threshold.min.b + threshold.max.b) / 2;
    
    const distance = Math.sqrt(
        Math.pow(r - centerR, 2) +
        Math.pow(g - centerG, 2) +
        Math.pow(b - centerB, 2)
    );
    
    return Math.max(0, 1 - distance / 255);
}

/**
 * Groupe les détections proches pour éviter les doublons
 */
function groupNearbyDetections(detections) {
    const grouped = [];
    const processed = new Set();
    
    detections.forEach((detection, index) => {
        if (processed.has(index)) return;
        
        const group = [detection];
        processed.add(index);
        
        // Chercher les détections proches de la même couleur
        detections.forEach((other, otherIndex) => {
            if (processed.has(otherIndex) || otherIndex === index) return;
            
            const distance = Math.sqrt(
                Math.pow(detection.x - other.x, 2) +
                Math.pow(detection.y - other.y, 2)
            );
            
            if (distance < 100 && detection.color === other.color) {
                group.push(other);
                processed.add(otherIndex);
            }
        });
        
        // Créer une détection groupée
        if (group.length > 0) {
            const avgX = group.reduce((sum, d) => sum + d.x, 0) / group.length;
            const avgY = group.reduce((sum, d) => sum + d.y, 0) / group.length;
            const maxConfidence = Math.max(...group.map(d => d.confidence));
            
            grouped.push({
                x: avgX,
                y: avgY,
                width: Math.max(...group.map(d => d.width)),
                height: Math.max(...group.map(d => d.height)),
                color: detection.color,
                label: detection.label,
                confidence: maxConfidence,
                count: group.length
            });
        }
    });
    
    return grouped;
}

/**
 * Dessine les détections sur le canvas overlay
 */
function drawDetections(canvas, detections) {
    const ctx = canvas.getContext('2d');
    
    // Nettoyer le canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    
    // Dessiner chaque détection
    detections.forEach(detection => {
        const colors = {
            red: '#ef4444',
            green: '#10b981',
            blue: '#3b82f6'
        };
        
        const color = colors[detection.color] || '#6b7280';
        
        // Dessiner le rectangle de détection
        ctx.strokeStyle = color;
        ctx.lineWidth = 3;
        ctx.strokeRect(
            detection.x - detection.width / 2,
            detection.y - detection.height / 2,
            detection.width,
            detection.height
        );
        
        // Dessiner le label
        ctx.fillStyle = color;
        ctx.font = 'bold 16px Arial';
        const text = `${detection.label} (${Math.round(detection.confidence * 100)}%)`;
        const textMetrics = ctx.measureText(text);
        
        // Fond pour le texte
        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(
            detection.x - detection.width / 2,
            detection.y - detection.height / 2 - 25,
            textMetrics.width + 10,
            20
        );
        
        // Texte
        ctx.fillStyle = color;
        ctx.fillText(
            text,
            detection.x - detection.width / 2 + 5,
            detection.y - detection.height / 2 - 8
        );
    });
}

/**
 * Traite les détections pour mise à jour des compteurs et API
 */
function processDetections(detections) {
    const currentTime = Date.now();
    
    // Compter les détections par couleur
    const currentCounts = { red: 0, green: 0, blue: 0 };
    
    detections.forEach(detection => {
        if (currentCounts.hasOwnProperty(detection.color)) {
            currentCounts[detection.color]++;
        }
    });
    
    // Mettre à jour les compteurs si il y a des nouvelles détections
    let hasNewDetections = false;
    
    Object.keys(currentCounts).forEach(color => {
        if (currentCounts[color] > 0) {
            detectionCounts[color] += currentCounts[color];
            hasNewDetections = true;
        }
    });
    
    if (hasNewDetections) {
        updateCountersDisplay();
        
        // Envoyer à l'API seulement si assez de temps s'est écoulé
        if (currentTime - lastDetectionTime >= CONFIG.DETECTION_INTERVAL) {
            sendDetectionsToAPI(detections);
            lastDetectionTime = currentTime;
        }
    }
}

// ===== COMMUNICATION API =====

/**
 * Envoie les détections à l'API backend
 */
async function sendDetectionsToAPI(detections) {
    for (const detection of detections) {
        try {
            const gId = generateGId(detection.color, detection.label);
            
            const response = await fetch(`${CONFIG.API_BASE_URL}/detection`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    g_id: gId,
                    object_type: detection.label,
                    color: detection.color
                })
            });

            if (response.ok) {
                const result = await response.json();
                if (result.success) {
                    console.log(`📤 Détection envoyée: ${detection.label} (${detection.color})`);
                    updateDetectionStatus(`Dernière détection: ${detection.label}`);
                }
            }
        } catch (error) {
            console.error('Erreur API:', error);
            updateDetectionStatus('Erreur API');
        }
    }
}

/**
 * Génère un G_ID unique pour une détection
 */
function generateGId(color, label) {
    const timestamp = Date.now();
    const cleanLabel = label.replace(/\s+/g, '_').toUpperCase();
    return `${color.toUpperCase()}_${cleanLabel}_${timestamp}`;
}

/**
 * Charge les détections récentes depuis l'API
 */
async function loadRecentDetections() {
    try {
        const response = await fetch(`${CONFIG.API_BASE_URL}/stats`);
        
        if (response.ok) {
            const result = await response.json();
            
            if (result.success) {
                updateRecentList(result.data.recent || []);
                updateStatsFromAPI(result.data);
            }
        }
    } catch (error) {
        console.error('Erreur chargement historique:', error);
        updateRecentList([]);
    }
}

/**
 * Met à jour la liste des détections récentes
 */
function updateRecentList(recentDetections) {
    const recentList = document.getElementById('recentList');
    
    if (!recentDetections || recentDetections.length === 0) {
        recentList.innerHTML = '<div class="loading">Aucune détection récente</div>';
        return;
    }
    
    const html = recentDetections.map(detection => `
        <div class="recent-item">
            <div class="color-dot ${detection.color}"></div>
            <div class="recent-info