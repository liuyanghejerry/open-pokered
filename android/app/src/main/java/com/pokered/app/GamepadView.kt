package com.pokered.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.PorterDuff
import android.graphics.RectF
import android.util.AttributeSet
import android.util.TypedValue
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView

/**
 * Virtual gamepad view matching the web version's layout.
 *
 * Layout:
 * ┌──────────────────────────────────────┐
 * │  ┌─────────┐          ┌──┐   ┌──┐  │  ← dpad (left) + A/B (right)
 * │  │    ▲    │          │B │   │A │  │
 * │  │  ◄ ■ ►  │          └──┘   └──┘  │
 * │  │    ▼    │                        │
 * │  └─────────┘                        │
 * │       [SELECT]    [START]           │  ← meta buttons (center)
 * └──────────────────────────────────────┘
 *
 * Button event codes match the GbButton enum in Rust (pokered-renderer/input.rs):
 *   0 = A, 1 = B, 2 = Select, 3 = Start, 4 = Right, 5 = Left, 6 = Up, 7 = Down
 */
class GamepadView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr), SurfaceHolder.Callback {

    // ── Button event codes (must match GbButton enum in input.rs) ──
    companion object {
        const val BTN_A = 0
        const val BTN_B = 1
        const val BTN_SELECT = 2
        const val BTN_START = 3
        const val BTN_RIGHT = 4
        const val BTN_LEFT = 5
        const val BTN_UP = 6
        const val BTN_DOWN = 7

        // Fixed button sizes in dp (human finger scale)
        private const val DPAD_ARM_LENGTH_DP = 56f
        private const val DPAD_ARM_WIDTH_DP = 32f
        private const val DPAD_GAP_DP = 8f
        private const val AB_BTN_RADIUS_DP = 34f
        private const val META_BTN_WIDTH_DP = 90f
        private const val META_BTN_HEIGHT_DP = 30f
        private const val AB_BTN_SPACING_DP = 16f
        private const val TEXT_SIZE_SP = 16f
        private const val ARROW_SIZE_SP = 14f
        private const val BORDER_WIDTH_DP = 2f
    }

    /** Callback: (buttonCode, pressed) */
    var onButtonEvent: ((Int, Boolean) -> Unit)? = null

    private var bottomInset: Int = 0

    /** True once the surface is created and safe to lock/draw. */
    private var surfaceReady = false

    init {
        // NativeActivity takes over the window surface for native (wgpu) rendering,
        // so a plain View overlay would never be composited. Give the gamepad its own
        // hardware layer placed *above* the game surface, with a translucent pixel
        // format so the transparent regions reveal the game behind it.
        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.TRANSLUCENT)
        holder.addCallback(this)
    }

    private val density: Float get() = resources.displayMetrics.density

    private fun spToPx(sp: Float): Float = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sp, resources.displayMetrics)

    private val dpadArmLengthPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, DPAD_ARM_LENGTH_DP, resources.displayMetrics)
    private val dpadArmWidthPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, DPAD_ARM_WIDTH_DP, resources.displayMetrics)
    private val dpadGapPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, DPAD_GAP_DP, resources.displayMetrics)
    private val abBtnRadiusPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, AB_BTN_RADIUS_DP, resources.displayMetrics)
    private val metaBtnWidthPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, META_BTN_WIDTH_DP, resources.displayMetrics)
    private val metaBtnHeightPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, META_BTN_HEIGHT_DP, resources.displayMetrics)
    private val abBtnSpacingPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, AB_BTN_SPACING_DP, resources.displayMetrics)
    private val borderWidthPx: Float get() = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, BORDER_WIDTH_DP, resources.displayMetrics)

    // ── Button regions (calculated in onLayout) ──
    private val dpadUpRect = RectF()
    private val dpadDownRect = RectF()
    private val dpadLeftRect = RectF()
    private val dpadRightRect = RectF()
    private val dpadCenterRect = RectF()
    private val btnARect = RectF()
    private val btnBRect = RectF()
    private val btnSelectRect = RectF()
    private val btnStartRect = RectF()

        private val activeButtons = mutableMapOf<Int, Int>()
        private val buttonActivity = BooleanArray(8)

    private val dpadPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#2a2a3e")
        style = Paint.Style.FILL
    }
    private val dpadPressedPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#ff4444")
        style = Paint.Style.FILL
    }
    private val dpadBorderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#3a3a5e")
        style = Paint.Style.STROKE
    }
    private val btnAPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#cc2222")
        style = Paint.Style.FILL
    }
    private val btnAPressedPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#ff4444")
        style = Paint.Style.FILL
    }
    private val btnBPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#cc2222")
        style = Paint.Style.FILL
    }
    private val btnBPressedPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#ff4444")
        style = Paint.Style.FILL
    }
    private val btnBorderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#ff6666")
        style = Paint.Style.STROKE
    }
    private val metaPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#333350")
        style = Paint.Style.FILL
    }
    private val metaPressedPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#ff4444")
        style = Paint.Style.FILL
    }
    private val metaTextPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#aaaaaa")
        textAlign = Paint.Align.CENTER
        isFakeBoldText = true
        letterSpacing = 0.05f
    }
    private val metaTextPressedPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textAlign = Paint.Align.CENTER
        isFakeBoldText = true
        letterSpacing = 0.05f
    }
    private val btnATextPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textAlign = Paint.Align.CENTER
        isFakeBoldText = true
    }
    private val btnBTextPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textAlign = Paint.Align.CENTER
        isFakeBoldText = true
    }
    private val dpadArrowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#cccccc")
        textAlign = Paint.Align.CENTER
    }

    fun setBottomInset(inset: Int) {
        bottomInset = inset
        requestLayout()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec)
        val heightMode = MeasureSpec.getMode(heightMeasureSpec)
        val heightSize = MeasureSpec.getSize(heightMeasureSpec)
        val desiredHeight = when (heightMode) {
            MeasureSpec.EXACTLY -> heightSize
            else -> {
                val armPx = dpadArmLengthPx + dpadGapPx
                val dpadH = armPx * 2f
                val metaH = metaBtnHeightPx
                val pad = 24f * density
                (dpadH + metaH + pad * 3f).toInt().coerceAtLeast(160)
            }
        }
        setMeasuredDimension(width, desiredHeight + bottomInset)
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        super.onLayout(changed, left, top, right, bottom)

        val w = (right - left).toFloat()
        val h = (bottom - top - bottomInset).toFloat()
        val cx = w / 2f
        val padding = 24f * density

        val armL = dpadArmLengthPx
        val armW = dpadArmWidthPx
        val gap = dpadGapPx
        val btnR = abBtnRadiusPx
        val spacing = abBtnSpacingPx

        val dpadCenterX = w * 0.33f
        val dpadCenterY = padding + armL

        dpadUpRect.set(dpadCenterX - armW / 2f, dpadCenterY - armL, dpadCenterX + armW / 2f, dpadCenterY - gap)
        dpadDownRect.set(dpadCenterX - armW / 2f, dpadCenterY + gap, dpadCenterX + armW / 2f, dpadCenterY + armL)
        dpadLeftRect.set(dpadCenterX - armL, dpadCenterY - armW / 2f, dpadCenterX - gap, dpadCenterY + armW / 2f)
        dpadRightRect.set(dpadCenterX + gap, dpadCenterY - armW / 2f, dpadCenterX + armL, dpadCenterY + armW / 2f)
        dpadCenterRect.set(dpadCenterX - gap, dpadCenterY - gap, dpadCenterX + gap, dpadCenterY + gap)

        val abCenterY = dpadCenterY
        val btnACenterX = w - padding - btnR
        val btnBCenterX = btnACenterX - btnR * 2f - spacing

        btnARect.set(btnACenterX - btnR, abCenterY - btnR, btnACenterX + btnR, abCenterY + btnR)
        btnBRect.set(btnBCenterX - btnR, abCenterY - btnR, btnBCenterX + btnR, abCenterY + btnR)

        val metaW = metaBtnWidthPx
        val metaH = metaBtnHeightPx
        val metaY = h - padding - metaH / 2f
        val metaGap = metaW * 0.3f

        btnSelectRect.set(cx - metaW - metaGap, metaY - metaH / 2f, cx - metaGap, metaY + metaH / 2f)
        btnStartRect.set(cx + metaGap, metaY - metaH / 2f, cx + metaW + metaGap, metaY + metaH / 2f)

        render()
    }

    // ── Surface lifecycle ──

    override fun surfaceCreated(holder: SurfaceHolder) {
        surfaceReady = true
        render()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        render()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        surfaceReady = false
    }

    /**
     * Draw the whole gamepad into the surface. Called on the UI thread from layout,
     * surface lifecycle, and button press/release — all low-frequency, so a software
     * lockCanvas is fine and no dedicated render thread is needed.
     */
    private fun render() {
        if (!surfaceReady || !holder.surface.isValid) return
        val canvas = holder.lockCanvas() ?: return
        try {
            drawGamepad(canvas)
        } finally {
            holder.unlockCanvasAndPost(canvas)
        }
    }

    private fun drawGamepad(canvas: Canvas) {
        btnATextPaint.textSize = spToPx(TEXT_SIZE_SP)
        btnBTextPaint.textSize = spToPx(TEXT_SIZE_SP)
        dpadArrowPaint.textSize = spToPx(ARROW_SIZE_SP)
        dpadBorderPaint.strokeWidth = borderWidthPx
        btnBorderPaint.strokeWidth = borderWidthPx

        // Clear the locked buffer to fully transparent so the game surface shows
        // through everywhere we don't paint a button (CLEAR ignores prior contents).
        canvas.drawColor(Color.TRANSPARENT, PorterDuff.Mode.CLEAR)

        drawDpadButton(canvas, dpadUpRect, BTN_UP)
        drawDpadButton(canvas, dpadDownRect, BTN_DOWN)
        drawDpadButton(canvas, dpadLeftRect, BTN_LEFT)
        drawDpadButton(canvas, dpadRightRect, BTN_RIGHT)
        drawDpadCenter(canvas)

        drawArrow(canvas, dpadUpRect, "▲")
        drawArrow(canvas, dpadDownRect, "▼")
        drawArrow(canvas, dpadLeftRect, "◀")
        drawArrow(canvas, dpadRightRect, "▶")

        drawActionButton(canvas, btnBRect, BTN_B, "B", btnBPaint, btnBPressedPaint)
        drawActionButton(canvas, btnARect, BTN_A, "A", btnAPaint, btnAPressedPaint)

        drawMetaButton(canvas, btnSelectRect, BTN_SELECT, "SELECT")
        drawMetaButton(canvas, btnStartRect, BTN_START, "START")
    }

    private fun drawDpadButton(canvas: Canvas, rect: RectF, buttonCode: Int) {
        val pressed = buttonActivity[buttonCode]
        val fillPaint = if (pressed) dpadPressedPaint else dpadPaint
        canvas.drawRect(rect, fillPaint)
        canvas.drawRect(rect, dpadBorderPaint)
    }

    private fun drawDpadCenter(canvas: Canvas) {
        canvas.drawRect(dpadCenterRect, dpadPaint)
        canvas.drawRect(dpadCenterRect, dpadBorderPaint)
    }

    private fun drawArrow(canvas: Canvas, rect: RectF, arrow: String) {
        canvas.drawText(arrow, rect.centerX(), rect.centerY() + dpadArrowPaint.textSize / 3f, dpadArrowPaint)
    }

    private fun drawActionButton(
        canvas: Canvas,
        rect: RectF,
        buttonCode: Int,
        label: String,
        normalPaint: Paint,
        pressedPaint: Paint
    ) {
        val pressed = buttonActivity[buttonCode]
        val fillPaint = if (pressed) pressedPaint else normalPaint
        canvas.drawOval(rect, fillPaint)
        if (!pressed) {
            canvas.drawOval(rect, btnBorderPaint)
        }
        canvas.drawText(label, rect.centerX(), rect.centerY() + btnATextPaint.textSize / 3f, btnATextPaint)
    }

    private fun drawMetaButton(canvas: Canvas, rect: RectF, buttonCode: Int, label: String) {
        val pressed = buttonActivity[buttonCode]
        val fillPaint = if (pressed) metaPressedPaint else metaPaint
        val textPaint = if (pressed) metaTextPressedPaint else metaTextPaint
        canvas.drawRoundRect(rect, rect.height() / 2f, rect.height() / 2f, fillPaint)
        canvas.drawRoundRect(rect, rect.height() / 2f, rect.height() / 2f, btnBorderPaint)
        textPaint.textSize = spToPx(12f)
        canvas.drawText(label, rect.centerX(), rect.centerY() + textPaint.textSize / 3f, textPaint)
    }

    // ── Touch handling ──

    private fun hitTest(x: Float, y: Float): Int? {
        if (btnARect.contains(x, y)) return BTN_A
        if (btnBRect.contains(x, y)) return BTN_B
        if (dpadUpRect.contains(x, y) && !dpadCenterRect.contains(x, y)) return BTN_UP
        if (dpadDownRect.contains(x, y) && !dpadCenterRect.contains(x, y)) return BTN_DOWN
        if (dpadLeftRect.contains(x, y) && !dpadCenterRect.contains(x, y)) return BTN_LEFT
        if (dpadRightRect.contains(x, y) && !dpadCenterRect.contains(x, y)) return BTN_RIGHT
        if (btnSelectRect.contains(x, y)) return BTN_SELECT
        if (btnStartRect.contains(x, y)) return BTN_START
        return null
    }

    private fun pressButton(buttonCode: Int) {
        if (!buttonActivity[buttonCode]) {
            buttonActivity[buttonCode] = true
            onButtonEvent?.invoke(buttonCode, true)
            render()
        }
    }

    private fun releaseButton(buttonCode: Int) {
        if (buttonActivity[buttonCode]) {
            buttonActivity[buttonCode] = false
            onButtonEvent?.invoke(buttonCode, false)
            render()
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val pointerIndex = event.actionIndex
        val pointerId = event.getPointerId(pointerIndex)
        val x = event.getX(pointerIndex)
        val y = event.getY(pointerIndex)

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN,
            MotionEvent.ACTION_POINTER_DOWN -> {
                val button = hitTest(x, y)
                if (button != null) {
                    activeButtons[pointerId] = button
                    pressButton(button)
                }
            }
            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    val pid = event.getPointerId(i)
                    val px = event.getX(i)
                    val py = event.getY(i)

                    val oldButton = activeButtons[pid]
                    val newButton = hitTest(px, py)

                    if (oldButton != null && oldButton != newButton) {
                        releaseButton(oldButton)
                        activeButtons.remove(pid)
                        if (newButton != null) {
                            activeButtons[pid] = newButton
                            pressButton(newButton)
                        }
                    }
                }
            }
            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_POINTER_UP,
            MotionEvent.ACTION_CANCEL -> {
                if (event.actionMasked == MotionEvent.ACTION_UP ||
                    event.actionMasked == MotionEvent.ACTION_CANCEL) {
                    for ((pid, btn) in activeButtons.toMap()) {
                        releaseButton(btn)
                        activeButtons.remove(pid)
                    }
                } else {
                    val btn = activeButtons[pointerId]
                    if (btn != null) {
                        releaseButton(btn)
                        activeButtons.remove(pointerId)
                    }
                }
            }
        }
        return true
    }
}
