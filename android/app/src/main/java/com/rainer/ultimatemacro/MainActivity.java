package com.rainer.ultimatemacro;

import android.Manifest;
import android.app.NativeActivity;
import android.content.Intent;
import android.content.pm.PackageManager;
import com.google.zxing.integration.android.IntentIntegrator;
import com.google.zxing.integration.android.IntentResult;

/** Slint's NativeActivity host. Camera preview/decoding stays entirely in Java. */
public class MainActivity extends NativeActivity {
    private static final int CAMERA_PERMISSION = 42;
    private boolean scanInProgress;
    private String scanResult;
    private long generation;

    // JNI calls originate on Slint's native thread; camera operations belong to
    // Android's main thread. Results are drained exactly once by the Slint timer.
    public void startScan(long requestGeneration) {
        runOnUiThread(() -> {
            if (scanInProgress) {
                synchronized (this) { scanResult = requestGeneration + "|ERROR:Camera is already open. Please retry."; }
                return;
            }
            generation = requestGeneration;
            scanInProgress = true;
            synchronized (this) { scanResult = null; }
            if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[]{Manifest.permission.CAMERA}, CAMERA_PERMISSION);
            } else {
                launchScanner();
            }
        });
    }

    private void launchScanner() {
        try {
            // NativeActivity cannot use AndroidX registerForActivityResult, so
            // use ZXing's IntentIntegrator and the platform result callback.
            new IntentIntegrator(this)
                .setDesiredBarcodeFormats(IntentIntegrator.PRODUCT_CODE_TYPES)
                .setPrompt("Scan the product barcode")
                .setBeepEnabled(false)
                .setOrientationLocked(false)
                .initiateScan();
        } catch (RuntimeException error) {
            finishScan("ERROR:Camera unavailable. Enter the barcode manually.");
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] grants) {
        super.onRequestPermissionsResult(requestCode, permissions, grants);
        if (requestCode != CAMERA_PERMISSION) return;
        if (grants.length > 0 && grants[0] == PackageManager.PERMISSION_GRANTED) {
            launchScanner();
        } else {
            finishScan("ERROR:Camera permission denied. Allow it in Android settings or enter the barcode manually.");
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        IntentResult result = IntentIntegrator.parseActivityResult(requestCode, resultCode, data);
        if (result == null) {
            super.onActivityResult(requestCode, resultCode, data);
            return;
        }
        finishScan(result.getContents() == null ? "CANCELLED" : "OK:" + result.getContents());
    }

    private synchronized void finishScan(String result) {
        scanInProgress = false;
        scanResult = generation + "|" + result;
    }

    public synchronized String takeScanResult() {
        String result = scanResult;
        scanResult = null;
        return result;
    }
}
