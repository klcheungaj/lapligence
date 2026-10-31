// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4: literal operands, so
// the default optimizer folds what it can while --no-opt evaluates at run time.
// Operand values follow arith_matrix.sv (indices 0, 1, 4, 5, 6, 10, 11).
// Width 1 of the four arith_constants_w*.sv fixtures: one width each keeps
// every process body small enough for C compilers to optimize quickly.
module tb;
  initial begin
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 0,
             -1'sh0, 1'sh0 + 1'sh0, 1'sh0 - 1'sh0, 1'sh0 * 1'sh0, 1'sh0 / 1'sh0,
             1'sh0 % 1'sh0, 1'sh0 ** 1'sh0, 1'h0 * 1'h0, 1'h0 / 1'h0, 1'h0 % 1'h0,
             1'h0 ** 1'h0, 1'sh0 / 1'h0, 1'sh0 ** 1'h0, 1'h0 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 0,
             1'h0 << 1'h0, 1'h0 >> 1'h0, 1'sh0 >>> 1'h0, 1'sh0 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 1,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 1,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 4,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 4,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 5,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 5,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 6,
             -1'sh0, 1'sh0 + 1'sh0, 1'sh0 - 1'sh0, 1'sh0 * 1'sh0, 1'sh0 / 1'sh0,
             1'sh0 % 1'sh0, 1'sh0 ** 1'sh0, 1'h0 * 1'h0, 1'h0 / 1'h0, 1'h0 % 1'h0,
             1'h0 ** 1'h0, 1'sh0 / 1'h0, 1'sh0 ** 1'h0, 1'h0 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 6,
             1'h0 << 1'h0, 1'h0 >> 1'h0, 1'sh0 >>> 1'h0, 1'sh0 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 10,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 10,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 0, 11,
             -1'sh0, 1'sh0 + 1'shx, 1'sh0 - 1'shx, 1'sh0 * 1'shx, 1'sh0 / 1'shx,
             1'sh0 % 1'shx, 1'sh0 ** 1'shx, 1'h0 * 1'hx, 1'h0 / 1'hx, 1'h0 % 1'hx,
             1'h0 ** 1'hx, 1'sh0 / 1'hx, 1'sh0 ** 1'hx, 1'h0 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 0, 11,
             1'h0 << 1'hx, 1'h0 >> 1'hx, 1'sh0 >>> 1'hx, 1'sh0 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 0,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 0,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 1,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 1,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 4,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 4,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 5,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 5,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 6,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 6,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 10,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 10,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 1, 11,
             -1'sh1, 1'sh1 + 1'shx, 1'sh1 - 1'shx, 1'sh1 * 1'shx, 1'sh1 / 1'shx,
             1'sh1 % 1'shx, 1'sh1 ** 1'shx, 1'h1 * 1'hx, 1'h1 / 1'hx, 1'h1 % 1'hx,
             1'h1 ** 1'hx, 1'sh1 / 1'hx, 1'sh1 ** 1'hx, 1'h1 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 1, 11,
             1'h1 << 1'hx, 1'h1 >> 1'hx, 1'sh1 >>> 1'hx, 1'sh1 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 0,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 0,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 1,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 1,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 4,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 4,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 5,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 5,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 6,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 6,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 10,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 10,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 4, 11,
             -1'sh1, 1'sh1 + 1'shx, 1'sh1 - 1'shx, 1'sh1 * 1'shx, 1'sh1 / 1'shx,
             1'sh1 % 1'shx, 1'sh1 ** 1'shx, 1'h1 * 1'hx, 1'h1 / 1'hx, 1'h1 % 1'hx,
             1'h1 ** 1'hx, 1'sh1 / 1'hx, 1'sh1 ** 1'hx, 1'h1 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 4, 11,
             1'h1 << 1'hx, 1'h1 >> 1'hx, 1'sh1 >>> 1'hx, 1'sh1 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 0,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 0,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 1,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 1,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 4,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 4,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 5,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 5,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 6,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 6,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 10,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 10,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 5, 11,
             -1'sh1, 1'sh1 + 1'shx, 1'sh1 - 1'shx, 1'sh1 * 1'shx, 1'sh1 / 1'shx,
             1'sh1 % 1'shx, 1'sh1 ** 1'shx, 1'h1 * 1'hx, 1'h1 / 1'hx, 1'h1 % 1'hx,
             1'h1 ** 1'hx, 1'sh1 / 1'hx, 1'sh1 ** 1'hx, 1'h1 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 5, 11,
             1'h1 << 1'hx, 1'h1 >> 1'hx, 1'sh1 >>> 1'hx, 1'sh1 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 0,
             -1'sh0, 1'sh0 + 1'sh0, 1'sh0 - 1'sh0, 1'sh0 * 1'sh0, 1'sh0 / 1'sh0,
             1'sh0 % 1'sh0, 1'sh0 ** 1'sh0, 1'h0 * 1'h0, 1'h0 / 1'h0, 1'h0 % 1'h0,
             1'h0 ** 1'h0, 1'sh0 / 1'h0, 1'sh0 ** 1'h0, 1'h0 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 0,
             1'h0 << 1'h0, 1'h0 >> 1'h0, 1'sh0 >>> 1'h0, 1'sh0 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 1,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 1,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 4,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 4,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 5,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 5,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 6,
             -1'sh0, 1'sh0 + 1'sh0, 1'sh0 - 1'sh0, 1'sh0 * 1'sh0, 1'sh0 / 1'sh0,
             1'sh0 % 1'sh0, 1'sh0 ** 1'sh0, 1'h0 * 1'h0, 1'h0 / 1'h0, 1'h0 % 1'h0,
             1'h0 ** 1'h0, 1'sh0 / 1'h0, 1'sh0 ** 1'h0, 1'h0 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 6,
             1'h0 << 1'h0, 1'h0 >> 1'h0, 1'sh0 >>> 1'h0, 1'sh0 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 10,
             -1'sh0, 1'sh0 + 1'sh1, 1'sh0 - 1'sh1, 1'sh0 * 1'sh1, 1'sh0 / 1'sh1,
             1'sh0 % 1'sh1, 1'sh0 ** 1'sh1, 1'h0 * 1'h1, 1'h0 / 1'h1, 1'h0 % 1'h1,
             1'h0 ** 1'h1, 1'sh0 / 1'h1, 1'sh0 ** 1'h1, 1'h0 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 10,
             1'h0 << 1'h1, 1'h0 >> 1'h1, 1'sh0 >>> 1'h1, 1'sh0 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 6, 11,
             -1'sh0, 1'sh0 + 1'shx, 1'sh0 - 1'shx, 1'sh0 * 1'shx, 1'sh0 / 1'shx,
             1'sh0 % 1'shx, 1'sh0 ** 1'shx, 1'h0 * 1'hx, 1'h0 / 1'hx, 1'h0 % 1'hx,
             1'h0 ** 1'hx, 1'sh0 / 1'hx, 1'sh0 ** 1'hx, 1'h0 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 6, 11,
             1'h0 << 1'hx, 1'h0 >> 1'hx, 1'sh0 >>> 1'hx, 1'sh0 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 0,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 0,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 1,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 1,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 4,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 4,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 5,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 5,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 6,
             -1'sh1, 1'sh1 + 1'sh0, 1'sh1 - 1'sh0, 1'sh1 * 1'sh0, 1'sh1 / 1'sh0,
             1'sh1 % 1'sh0, 1'sh1 ** 1'sh0, 1'h1 * 1'h0, 1'h1 / 1'h0, 1'h1 % 1'h0,
             1'h1 ** 1'h0, 1'sh1 / 1'h0, 1'sh1 ** 1'h0, 1'h1 ** 1'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 6,
             1'h1 << 1'h0, 1'h1 >> 1'h0, 1'sh1 >>> 1'h0, 1'sh1 >>> 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 10,
             -1'sh1, 1'sh1 + 1'sh1, 1'sh1 - 1'sh1, 1'sh1 * 1'sh1, 1'sh1 / 1'sh1,
             1'sh1 % 1'sh1, 1'sh1 ** 1'sh1, 1'h1 * 1'h1, 1'h1 / 1'h1, 1'h1 % 1'h1,
             1'h1 ** 1'h1, 1'sh1 / 1'h1, 1'sh1 ** 1'h1, 1'h1 ** 1'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 10,
             1'h1 << 1'h1, 1'h1 >> 1'h1, 1'sh1 >>> 1'h1, 1'sh1 >>> 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 10, 11,
             -1'sh1, 1'sh1 + 1'shx, 1'sh1 - 1'shx, 1'sh1 * 1'shx, 1'sh1 / 1'shx,
             1'sh1 % 1'shx, 1'sh1 ** 1'shx, 1'h1 * 1'hx, 1'h1 / 1'hx, 1'h1 % 1'hx,
             1'h1 ** 1'hx, 1'sh1 / 1'hx, 1'sh1 ** 1'hx, 1'h1 ** 1'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 1, 10, 11,
             1'h1 << 1'hx, 1'h1 >> 1'hx, 1'sh1 >>> 1'hx, 1'sh1 >>> 1'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 0,
             -1'shx, 1'shx + 1'sh0, 1'shx - 1'sh0, 1'shx * 1'sh0, 1'shx / 1'sh0,
             1'shx % 1'sh0, 1'shx ** 1'sh0, 1'hx * 1'h0, 1'hx / 1'h0, 1'hx % 1'h0,
             1'hx ** 1'h0, 1'shx / 1'h0, 1'shx ** 1'h0, 1'hx ** 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 1,
             -1'shx, 1'shx + 1'sh1, 1'shx - 1'sh1, 1'shx * 1'sh1, 1'shx / 1'sh1,
             1'shx % 1'sh1, 1'shx ** 1'sh1, 1'hx * 1'h1, 1'hx / 1'h1, 1'hx % 1'h1,
             1'hx ** 1'h1, 1'shx / 1'h1, 1'shx ** 1'h1, 1'hx ** 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 4,
             -1'shx, 1'shx + 1'sh1, 1'shx - 1'sh1, 1'shx * 1'sh1, 1'shx / 1'sh1,
             1'shx % 1'sh1, 1'shx ** 1'sh1, 1'hx * 1'h1, 1'hx / 1'h1, 1'hx % 1'h1,
             1'hx ** 1'h1, 1'shx / 1'h1, 1'shx ** 1'h1, 1'hx ** 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 5,
             -1'shx, 1'shx + 1'sh1, 1'shx - 1'sh1, 1'shx * 1'sh1, 1'shx / 1'sh1,
             1'shx % 1'sh1, 1'shx ** 1'sh1, 1'hx * 1'h1, 1'hx / 1'h1, 1'hx % 1'h1,
             1'hx ** 1'h1, 1'shx / 1'h1, 1'shx ** 1'h1, 1'hx ** 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 6,
             -1'shx, 1'shx + 1'sh0, 1'shx - 1'sh0, 1'shx * 1'sh0, 1'shx / 1'sh0,
             1'shx % 1'sh0, 1'shx ** 1'sh0, 1'hx * 1'h0, 1'hx / 1'h0, 1'hx % 1'h0,
             1'hx ** 1'h0, 1'shx / 1'h0, 1'shx ** 1'h0, 1'hx ** 1'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 10,
             -1'shx, 1'shx + 1'sh1, 1'shx - 1'sh1, 1'shx * 1'sh1, 1'shx / 1'sh1,
             1'shx % 1'sh1, 1'shx ** 1'sh1, 1'hx * 1'h1, 1'hx / 1'h1, 1'hx % 1'h1,
             1'hx ** 1'h1, 1'shx / 1'h1, 1'shx ** 1'h1, 1'hx ** 1'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 1, 11, 11,
             -1'shx, 1'shx + 1'shx, 1'shx - 1'shx, 1'shx * 1'shx, 1'shx / 1'shx,
             1'shx % 1'shx, 1'shx ** 1'shx, 1'hx * 1'hx, 1'hx / 1'hx, 1'hx % 1'hx,
             1'hx ** 1'hx, 1'shx / 1'hx, 1'shx ** 1'hx, 1'hx ** 1'shx);
    $finish(0);
  end
endmodule
