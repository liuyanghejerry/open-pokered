package com.dotzuki.player;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewGroup;

/** Native-view regression harness. Run against an exported debug host with a game pack. */
public final class TouchInstrumentation extends Instrumentation {
    private MainActivity activity;
    private GamepadView pad;
    private GameSurfaceView game;
    private float density;
    private final long downTime = SystemClock.uptimeMillis();

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            Intent intent = new Intent(Intent.ACTION_MAIN);
            intent.setClassName(getTargetContext(), MainActivity.class.getName());
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
            activity = (MainActivity) startActivitySync(intent);
            waitForIdleSync();
            final Throwable[] failure = new Throwable[1];
            runOnMainSync(() -> {
              try {
                ViewGroup content = activity.findViewById(android.R.id.content);
                ViewGroup root = (ViewGroup) content.getChildAt(0);
                game = (GameSurfaceView) root.getChildAt(0);
                pad = (GamepadView) root.getChildAt(1);
                density = activity.getResources().getDisplayMetrics().density;
                if (game.getHeight() != pad.getHeight()) throw new AssertionError("controls reserve game height");
                checkGestures();
              } catch (Throwable error) { failure[0] = error; }
            });
            if (failure[0] != null) throw failure[0];
            result.putString("stream", "PASS: full-window overlay, multi-pointer routing, focus cancellation, resize cancellation\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "FAIL: " + error + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
    private void checkGestures() {
        float[] layout = NativeBridge.INSTANCE.touchLayout();
        float px = layout[1] + layout[3] / 2, py = layout[2] + layout[4] / 2;
        float ax = layout[6] + layout[8] / 2, ay = layout[7] + layout[9] / 2;
        pad.releaseAll();
        send(MotionEvent.ACTION_DOWN, new int[]{11}, new float[]{px + 40, py});
        expect(16);
        send(MotionEvent.ACTION_POINTER_DOWN | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT),
            new int[]{11, 29}, new float[]{px + 40, py, ax, ay}); expect(17);
        send(MotionEvent.ACTION_MOVE, new int[]{11, 29}, new float[]{px, py - 40, ax, ay}); expect(65);
        // Release first pointer; the remaining index changes, but its ID owns A.
        send(MotionEvent.ACTION_POINTER_UP, new int[]{11, 29}, new float[]{px, py - 40, ax, ay}); expect(1);
        send(MotionEvent.ACTION_MOVE, new int[]{29}, new float[]{ax, ay}); expect(1);
        send(MotionEvent.ACTION_UP, new int[]{29}, new float[]{ax, ay}); expect(0);
        send(MotionEvent.ACTION_DOWN, new int[]{7}, new float[]{px + 40, py}); expect(16);
        activity.onWindowFocusChanged(false); expect(0);
        activity.onWindowFocusChanged(true);
        send(MotionEvent.ACTION_MOVE, new int[]{7}, new float[]{px + 40, py}); expect(0);
        send(MotionEvent.ACTION_UP, new int[]{7}, new float[]{px + 40, py});
        send(MotionEvent.ACTION_DOWN, new int[]{8}, new float[]{px + 40, py}); expect(16);
        send(MotionEvent.ACTION_CANCEL, new int[]{8}, new float[]{px + 40, py}); expect(0);
        send(MotionEvent.ACTION_MOVE, new int[]{8}, new float[]{px + 40, py}); expect(0);
        int width = pad.getWidth(), height = pad.getHeight();
        send(MotionEvent.ACTION_DOWN, new int[]{9}, new float[]{px + 40, py}); expect(16);
        pad.layout(0, 0, width - 1, height); expect(0);
        send(MotionEvent.ACTION_MOVE, new int[]{9}, new float[]{px + 40, py}); expect(0);
        pad.layout(0, 0, width, height);
    }
    private void expect(int expected) {
        if (game.getInputBits() != expected) throw new AssertionError("expected " + expected + ", got " + game.getInputBits());
    }
    private void send(int action, int[] ids, float[] positions) {
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[ids.length];
        MotionEvent.PointerCoords[] coordinates = new MotionEvent.PointerCoords[ids.length];
        for (int i = 0; i < ids.length; ++i) {
            properties[i] = new MotionEvent.PointerProperties(); properties[i].id = ids[i];
            properties[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coordinates[i] = new MotionEvent.PointerCoords();
            coordinates[i].x = positions[i * 2] * density;
            coordinates[i].y = positions[i * 2 + 1] * density;
            coordinates[i].pressure = 1; coordinates[i].size = 1;
        }
        MotionEvent event = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, ids.length,
            properties, coordinates, 0, 0, 1, 1, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
        pad.dispatchTouchEvent(event); event.recycle();
    }
}
