// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4: literal operands, so
// the default optimizer folds what it can while --no-opt evaluates at run time.
// Operand values follow arith_matrix.sv (indices 0, 1, 4, 5, 6, 10, 11).
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
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 0,
             -32'sh0, 32'sh0 + 32'sh0, 32'sh0 - 32'sh0, 32'sh0 * 32'sh0, 32'sh0 / 32'sh0,
             32'sh0 % 32'sh0, 32'sh0 ** 32'sh0, 32'h0 * 32'h0, 32'h0 / 32'h0, 32'h0 % 32'h0,
             32'h0 ** 32'h0, 32'sh0 / 32'h0, 32'sh0 ** 32'h0, 32'h0 ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 0,
             32'h0 << 32'h0, 32'h0 >> 32'h0, 32'sh0 >>> 32'h0, 32'sh0 >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 1,
             -32'sh0, 32'sh0 + 32'sh1, 32'sh0 - 32'sh1, 32'sh0 * 32'sh1, 32'sh0 / 32'sh1,
             32'sh0 % 32'sh1, 32'sh0 ** 32'sh1, 32'h0 * 32'h1, 32'h0 / 32'h1, 32'h0 % 32'h1,
             32'h0 ** 32'h1, 32'sh0 / 32'h1, 32'sh0 ** 32'h1, 32'h0 ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 1,
             32'h0 << 32'h1, 32'h0 >> 32'h1, 32'sh0 >>> 32'h1, 32'sh0 >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 4,
             -32'sh0, 32'sh0 + 32'shffffffff, 32'sh0 - 32'shffffffff, 32'sh0 * 32'shffffffff, 32'sh0 / 32'shffffffff,
             32'sh0 % 32'shffffffff, 32'sh0 ** 32'shffffffff, 32'h0 * 32'hffffffff, 32'h0 / 32'hffffffff, 32'h0 % 32'hffffffff,
             32'h0 ** 32'hffffffff, 32'sh0 / 32'hffffffff, 32'sh0 ** 32'hffffffff, 32'h0 ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 4,
             32'h0 << 32'hffffffff, 32'h0 >> 32'hffffffff, 32'sh0 >>> 32'hffffffff, 32'sh0 >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 5,
             -32'sh0, 32'sh0 + 32'sh80000000, 32'sh0 - 32'sh80000000, 32'sh0 * 32'sh80000000, 32'sh0 / 32'sh80000000,
             32'sh0 % 32'sh80000000, 32'sh0 ** 32'sh80000000, 32'h0 * 32'h80000000, 32'h0 / 32'h80000000, 32'h0 % 32'h80000000,
             32'h0 ** 32'h80000000, 32'sh0 / 32'h80000000, 32'sh0 ** 32'h80000000, 32'h0 ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 5,
             32'h0 << 32'h80000000, 32'h0 >> 32'h80000000, 32'sh0 >>> 32'h80000000, 32'sh0 >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 6,
             -32'sh0, 32'sh0 + 32'sh7fffffff, 32'sh0 - 32'sh7fffffff, 32'sh0 * 32'sh7fffffff, 32'sh0 / 32'sh7fffffff,
             32'sh0 % 32'sh7fffffff, 32'sh0 ** 32'sh7fffffff, 32'h0 * 32'h7fffffff, 32'h0 / 32'h7fffffff, 32'h0 % 32'h7fffffff,
             32'h0 ** 32'h7fffffff, 32'sh0 / 32'h7fffffff, 32'sh0 ** 32'h7fffffff, 32'h0 ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 6,
             32'h0 << 32'h7fffffff, 32'h0 >> 32'h7fffffff, 32'sh0 >>> 32'h7fffffff, 32'sh0 >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 10,
             -32'sh0, 32'sh0 + 32'shfffffffd, 32'sh0 - 32'shfffffffd, 32'sh0 * 32'shfffffffd, 32'sh0 / 32'shfffffffd,
             32'sh0 % 32'shfffffffd, 32'sh0 ** 32'shfffffffd, 32'h0 * 32'hfffffffd, 32'h0 / 32'hfffffffd, 32'h0 % 32'hfffffffd,
             32'h0 ** 32'hfffffffd, 32'sh0 / 32'hfffffffd, 32'sh0 ** 32'hfffffffd, 32'h0 ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 10,
             32'h0 << 32'hfffffffd, 32'h0 >> 32'hfffffffd, 32'sh0 >>> 32'hfffffffd, 32'sh0 >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 0, 11,
             -32'sh0, 32'sh0 + 32'shx, 32'sh0 - 32'shx, 32'sh0 * 32'shx, 32'sh0 / 32'shx,
             32'sh0 % 32'shx, 32'sh0 ** 32'shx, 32'h0 * 32'hx, 32'h0 / 32'hx, 32'h0 % 32'hx,
             32'h0 ** 32'hx, 32'sh0 / 32'hx, 32'sh0 ** 32'hx, 32'h0 ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 0, 11,
             32'h0 << 32'hx, 32'h0 >> 32'hx, 32'sh0 >>> 32'hx, 32'sh0 >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 0,
             -32'sh1, 32'sh1 + 32'sh0, 32'sh1 - 32'sh0, 32'sh1 * 32'sh0, 32'sh1 / 32'sh0,
             32'sh1 % 32'sh0, 32'sh1 ** 32'sh0, 32'h1 * 32'h0, 32'h1 / 32'h0, 32'h1 % 32'h0,
             32'h1 ** 32'h0, 32'sh1 / 32'h0, 32'sh1 ** 32'h0, 32'h1 ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 0,
             32'h1 << 32'h0, 32'h1 >> 32'h0, 32'sh1 >>> 32'h0, 32'sh1 >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 1,
             -32'sh1, 32'sh1 + 32'sh1, 32'sh1 - 32'sh1, 32'sh1 * 32'sh1, 32'sh1 / 32'sh1,
             32'sh1 % 32'sh1, 32'sh1 ** 32'sh1, 32'h1 * 32'h1, 32'h1 / 32'h1, 32'h1 % 32'h1,
             32'h1 ** 32'h1, 32'sh1 / 32'h1, 32'sh1 ** 32'h1, 32'h1 ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 1,
             32'h1 << 32'h1, 32'h1 >> 32'h1, 32'sh1 >>> 32'h1, 32'sh1 >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 4,
             -32'sh1, 32'sh1 + 32'shffffffff, 32'sh1 - 32'shffffffff, 32'sh1 * 32'shffffffff, 32'sh1 / 32'shffffffff,
             32'sh1 % 32'shffffffff, 32'sh1 ** 32'shffffffff, 32'h1 * 32'hffffffff, 32'h1 / 32'hffffffff, 32'h1 % 32'hffffffff,
             32'h1 ** 32'hffffffff, 32'sh1 / 32'hffffffff, 32'sh1 ** 32'hffffffff, 32'h1 ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 4,
             32'h1 << 32'hffffffff, 32'h1 >> 32'hffffffff, 32'sh1 >>> 32'hffffffff, 32'sh1 >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 5,
             -32'sh1, 32'sh1 + 32'sh80000000, 32'sh1 - 32'sh80000000, 32'sh1 * 32'sh80000000, 32'sh1 / 32'sh80000000,
             32'sh1 % 32'sh80000000, 32'sh1 ** 32'sh80000000, 32'h1 * 32'h80000000, 32'h1 / 32'h80000000, 32'h1 % 32'h80000000,
             32'h1 ** 32'h80000000, 32'sh1 / 32'h80000000, 32'sh1 ** 32'h80000000, 32'h1 ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 5,
             32'h1 << 32'h80000000, 32'h1 >> 32'h80000000, 32'sh1 >>> 32'h80000000, 32'sh1 >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 6,
             -32'sh1, 32'sh1 + 32'sh7fffffff, 32'sh1 - 32'sh7fffffff, 32'sh1 * 32'sh7fffffff, 32'sh1 / 32'sh7fffffff,
             32'sh1 % 32'sh7fffffff, 32'sh1 ** 32'sh7fffffff, 32'h1 * 32'h7fffffff, 32'h1 / 32'h7fffffff, 32'h1 % 32'h7fffffff,
             32'h1 ** 32'h7fffffff, 32'sh1 / 32'h7fffffff, 32'sh1 ** 32'h7fffffff, 32'h1 ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 6,
             32'h1 << 32'h7fffffff, 32'h1 >> 32'h7fffffff, 32'sh1 >>> 32'h7fffffff, 32'sh1 >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 10,
             -32'sh1, 32'sh1 + 32'shfffffffd, 32'sh1 - 32'shfffffffd, 32'sh1 * 32'shfffffffd, 32'sh1 / 32'shfffffffd,
             32'sh1 % 32'shfffffffd, 32'sh1 ** 32'shfffffffd, 32'h1 * 32'hfffffffd, 32'h1 / 32'hfffffffd, 32'h1 % 32'hfffffffd,
             32'h1 ** 32'hfffffffd, 32'sh1 / 32'hfffffffd, 32'sh1 ** 32'hfffffffd, 32'h1 ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 10,
             32'h1 << 32'hfffffffd, 32'h1 >> 32'hfffffffd, 32'sh1 >>> 32'hfffffffd, 32'sh1 >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 1, 11,
             -32'sh1, 32'sh1 + 32'shx, 32'sh1 - 32'shx, 32'sh1 * 32'shx, 32'sh1 / 32'shx,
             32'sh1 % 32'shx, 32'sh1 ** 32'shx, 32'h1 * 32'hx, 32'h1 / 32'hx, 32'h1 % 32'hx,
             32'h1 ** 32'hx, 32'sh1 / 32'hx, 32'sh1 ** 32'hx, 32'h1 ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 1, 11,
             32'h1 << 32'hx, 32'h1 >> 32'hx, 32'sh1 >>> 32'hx, 32'sh1 >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 0,
             -32'shffffffff, 32'shffffffff + 32'sh0, 32'shffffffff - 32'sh0, 32'shffffffff * 32'sh0, 32'shffffffff / 32'sh0,
             32'shffffffff % 32'sh0, 32'shffffffff ** 32'sh0, 32'hffffffff * 32'h0, 32'hffffffff / 32'h0, 32'hffffffff % 32'h0,
             32'hffffffff ** 32'h0, 32'shffffffff / 32'h0, 32'shffffffff ** 32'h0, 32'hffffffff ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 0,
             32'hffffffff << 32'h0, 32'hffffffff >> 32'h0, 32'shffffffff >>> 32'h0, 32'shffffffff >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 1,
             -32'shffffffff, 32'shffffffff + 32'sh1, 32'shffffffff - 32'sh1, 32'shffffffff * 32'sh1, 32'shffffffff / 32'sh1,
             32'shffffffff % 32'sh1, 32'shffffffff ** 32'sh1, 32'hffffffff * 32'h1, 32'hffffffff / 32'h1, 32'hffffffff % 32'h1,
             32'hffffffff ** 32'h1, 32'shffffffff / 32'h1, 32'shffffffff ** 32'h1, 32'hffffffff ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 1,
             32'hffffffff << 32'h1, 32'hffffffff >> 32'h1, 32'shffffffff >>> 32'h1, 32'shffffffff >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 4,
             -32'shffffffff, 32'shffffffff + 32'shffffffff, 32'shffffffff - 32'shffffffff, 32'shffffffff * 32'shffffffff, 32'shffffffff / 32'shffffffff,
             32'shffffffff % 32'shffffffff, 32'shffffffff ** 32'shffffffff, 32'hffffffff * 32'hffffffff, 32'hffffffff / 32'hffffffff, 32'hffffffff % 32'hffffffff,
             32'hffffffff ** 32'hffffffff, 32'shffffffff / 32'hffffffff, 32'shffffffff ** 32'hffffffff, 32'hffffffff ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 4,
             32'hffffffff << 32'hffffffff, 32'hffffffff >> 32'hffffffff, 32'shffffffff >>> 32'hffffffff, 32'shffffffff >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 5,
             -32'shffffffff, 32'shffffffff + 32'sh80000000, 32'shffffffff - 32'sh80000000, 32'shffffffff * 32'sh80000000, 32'shffffffff / 32'sh80000000,
             32'shffffffff % 32'sh80000000, 32'shffffffff ** 32'sh80000000, 32'hffffffff * 32'h80000000, 32'hffffffff / 32'h80000000, 32'hffffffff % 32'h80000000,
             32'hffffffff ** 32'h80000000, 32'shffffffff / 32'h80000000, 32'shffffffff ** 32'h80000000, 32'hffffffff ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 5,
             32'hffffffff << 32'h80000000, 32'hffffffff >> 32'h80000000, 32'shffffffff >>> 32'h80000000, 32'shffffffff >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 6,
             -32'shffffffff, 32'shffffffff + 32'sh7fffffff, 32'shffffffff - 32'sh7fffffff, 32'shffffffff * 32'sh7fffffff, 32'shffffffff / 32'sh7fffffff,
             32'shffffffff % 32'sh7fffffff, 32'shffffffff ** 32'sh7fffffff, 32'hffffffff * 32'h7fffffff, 32'hffffffff / 32'h7fffffff, 32'hffffffff % 32'h7fffffff,
             32'hffffffff ** 32'h7fffffff, 32'shffffffff / 32'h7fffffff, 32'shffffffff ** 32'h7fffffff, 32'hffffffff ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 6,
             32'hffffffff << 32'h7fffffff, 32'hffffffff >> 32'h7fffffff, 32'shffffffff >>> 32'h7fffffff, 32'shffffffff >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 10,
             -32'shffffffff, 32'shffffffff + 32'shfffffffd, 32'shffffffff - 32'shfffffffd, 32'shffffffff * 32'shfffffffd, 32'shffffffff / 32'shfffffffd,
             32'shffffffff % 32'shfffffffd, 32'shffffffff ** 32'shfffffffd, 32'hffffffff * 32'hfffffffd, 32'hffffffff / 32'hfffffffd, 32'hffffffff % 32'hfffffffd,
             32'hffffffff ** 32'hfffffffd, 32'shffffffff / 32'hfffffffd, 32'shffffffff ** 32'hfffffffd, 32'hffffffff ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 10,
             32'hffffffff << 32'hfffffffd, 32'hffffffff >> 32'hfffffffd, 32'shffffffff >>> 32'hfffffffd, 32'shffffffff >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 4, 11,
             -32'shffffffff, 32'shffffffff + 32'shx, 32'shffffffff - 32'shx, 32'shffffffff * 32'shx, 32'shffffffff / 32'shx,
             32'shffffffff % 32'shx, 32'shffffffff ** 32'shx, 32'hffffffff * 32'hx, 32'hffffffff / 32'hx, 32'hffffffff % 32'hx,
             32'hffffffff ** 32'hx, 32'shffffffff / 32'hx, 32'shffffffff ** 32'hx, 32'hffffffff ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 4, 11,
             32'hffffffff << 32'hx, 32'hffffffff >> 32'hx, 32'shffffffff >>> 32'hx, 32'shffffffff >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 0,
             -32'sh80000000, 32'sh80000000 + 32'sh0, 32'sh80000000 - 32'sh0, 32'sh80000000 * 32'sh0, 32'sh80000000 / 32'sh0,
             32'sh80000000 % 32'sh0, 32'sh80000000 ** 32'sh0, 32'h80000000 * 32'h0, 32'h80000000 / 32'h0, 32'h80000000 % 32'h0,
             32'h80000000 ** 32'h0, 32'sh80000000 / 32'h0, 32'sh80000000 ** 32'h0, 32'h80000000 ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 0,
             32'h80000000 << 32'h0, 32'h80000000 >> 32'h0, 32'sh80000000 >>> 32'h0, 32'sh80000000 >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 1,
             -32'sh80000000, 32'sh80000000 + 32'sh1, 32'sh80000000 - 32'sh1, 32'sh80000000 * 32'sh1, 32'sh80000000 / 32'sh1,
             32'sh80000000 % 32'sh1, 32'sh80000000 ** 32'sh1, 32'h80000000 * 32'h1, 32'h80000000 / 32'h1, 32'h80000000 % 32'h1,
             32'h80000000 ** 32'h1, 32'sh80000000 / 32'h1, 32'sh80000000 ** 32'h1, 32'h80000000 ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 1,
             32'h80000000 << 32'h1, 32'h80000000 >> 32'h1, 32'sh80000000 >>> 32'h1, 32'sh80000000 >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 4,
             -32'sh80000000, 32'sh80000000 + 32'shffffffff, 32'sh80000000 - 32'shffffffff, 32'sh80000000 * 32'shffffffff, 32'sh80000000 / 32'shffffffff,
             32'sh80000000 % 32'shffffffff, 32'sh80000000 ** 32'shffffffff, 32'h80000000 * 32'hffffffff, 32'h80000000 / 32'hffffffff, 32'h80000000 % 32'hffffffff,
             32'h80000000 ** 32'hffffffff, 32'sh80000000 / 32'hffffffff, 32'sh80000000 ** 32'hffffffff, 32'h80000000 ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 4,
             32'h80000000 << 32'hffffffff, 32'h80000000 >> 32'hffffffff, 32'sh80000000 >>> 32'hffffffff, 32'sh80000000 >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 5,
             -32'sh80000000, 32'sh80000000 + 32'sh80000000, 32'sh80000000 - 32'sh80000000, 32'sh80000000 * 32'sh80000000, 32'sh80000000 / 32'sh80000000,
             32'sh80000000 % 32'sh80000000, 32'sh80000000 ** 32'sh80000000, 32'h80000000 * 32'h80000000, 32'h80000000 / 32'h80000000, 32'h80000000 % 32'h80000000,
             32'h80000000 ** 32'h80000000, 32'sh80000000 / 32'h80000000, 32'sh80000000 ** 32'h80000000, 32'h80000000 ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 5,
             32'h80000000 << 32'h80000000, 32'h80000000 >> 32'h80000000, 32'sh80000000 >>> 32'h80000000, 32'sh80000000 >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 6,
             -32'sh80000000, 32'sh80000000 + 32'sh7fffffff, 32'sh80000000 - 32'sh7fffffff, 32'sh80000000 * 32'sh7fffffff, 32'sh80000000 / 32'sh7fffffff,
             32'sh80000000 % 32'sh7fffffff, 32'sh80000000 ** 32'sh7fffffff, 32'h80000000 * 32'h7fffffff, 32'h80000000 / 32'h7fffffff, 32'h80000000 % 32'h7fffffff,
             32'h80000000 ** 32'h7fffffff, 32'sh80000000 / 32'h7fffffff, 32'sh80000000 ** 32'h7fffffff, 32'h80000000 ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 6,
             32'h80000000 << 32'h7fffffff, 32'h80000000 >> 32'h7fffffff, 32'sh80000000 >>> 32'h7fffffff, 32'sh80000000 >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 10,
             -32'sh80000000, 32'sh80000000 + 32'shfffffffd, 32'sh80000000 - 32'shfffffffd, 32'sh80000000 * 32'shfffffffd, 32'sh80000000 / 32'shfffffffd,
             32'sh80000000 % 32'shfffffffd, 32'sh80000000 ** 32'shfffffffd, 32'h80000000 * 32'hfffffffd, 32'h80000000 / 32'hfffffffd, 32'h80000000 % 32'hfffffffd,
             32'h80000000 ** 32'hfffffffd, 32'sh80000000 / 32'hfffffffd, 32'sh80000000 ** 32'hfffffffd, 32'h80000000 ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 10,
             32'h80000000 << 32'hfffffffd, 32'h80000000 >> 32'hfffffffd, 32'sh80000000 >>> 32'hfffffffd, 32'sh80000000 >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 5, 11,
             -32'sh80000000, 32'sh80000000 + 32'shx, 32'sh80000000 - 32'shx, 32'sh80000000 * 32'shx, 32'sh80000000 / 32'shx,
             32'sh80000000 % 32'shx, 32'sh80000000 ** 32'shx, 32'h80000000 * 32'hx, 32'h80000000 / 32'hx, 32'h80000000 % 32'hx,
             32'h80000000 ** 32'hx, 32'sh80000000 / 32'hx, 32'sh80000000 ** 32'hx, 32'h80000000 ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 5, 11,
             32'h80000000 << 32'hx, 32'h80000000 >> 32'hx, 32'sh80000000 >>> 32'hx, 32'sh80000000 >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 0,
             -32'sh7fffffff, 32'sh7fffffff + 32'sh0, 32'sh7fffffff - 32'sh0, 32'sh7fffffff * 32'sh0, 32'sh7fffffff / 32'sh0,
             32'sh7fffffff % 32'sh0, 32'sh7fffffff ** 32'sh0, 32'h7fffffff * 32'h0, 32'h7fffffff / 32'h0, 32'h7fffffff % 32'h0,
             32'h7fffffff ** 32'h0, 32'sh7fffffff / 32'h0, 32'sh7fffffff ** 32'h0, 32'h7fffffff ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 0,
             32'h7fffffff << 32'h0, 32'h7fffffff >> 32'h0, 32'sh7fffffff >>> 32'h0, 32'sh7fffffff >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 1,
             -32'sh7fffffff, 32'sh7fffffff + 32'sh1, 32'sh7fffffff - 32'sh1, 32'sh7fffffff * 32'sh1, 32'sh7fffffff / 32'sh1,
             32'sh7fffffff % 32'sh1, 32'sh7fffffff ** 32'sh1, 32'h7fffffff * 32'h1, 32'h7fffffff / 32'h1, 32'h7fffffff % 32'h1,
             32'h7fffffff ** 32'h1, 32'sh7fffffff / 32'h1, 32'sh7fffffff ** 32'h1, 32'h7fffffff ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 1,
             32'h7fffffff << 32'h1, 32'h7fffffff >> 32'h1, 32'sh7fffffff >>> 32'h1, 32'sh7fffffff >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 4,
             -32'sh7fffffff, 32'sh7fffffff + 32'shffffffff, 32'sh7fffffff - 32'shffffffff, 32'sh7fffffff * 32'shffffffff, 32'sh7fffffff / 32'shffffffff,
             32'sh7fffffff % 32'shffffffff, 32'sh7fffffff ** 32'shffffffff, 32'h7fffffff * 32'hffffffff, 32'h7fffffff / 32'hffffffff, 32'h7fffffff % 32'hffffffff,
             32'h7fffffff ** 32'hffffffff, 32'sh7fffffff / 32'hffffffff, 32'sh7fffffff ** 32'hffffffff, 32'h7fffffff ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 4,
             32'h7fffffff << 32'hffffffff, 32'h7fffffff >> 32'hffffffff, 32'sh7fffffff >>> 32'hffffffff, 32'sh7fffffff >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 5,
             -32'sh7fffffff, 32'sh7fffffff + 32'sh80000000, 32'sh7fffffff - 32'sh80000000, 32'sh7fffffff * 32'sh80000000, 32'sh7fffffff / 32'sh80000000,
             32'sh7fffffff % 32'sh80000000, 32'sh7fffffff ** 32'sh80000000, 32'h7fffffff * 32'h80000000, 32'h7fffffff / 32'h80000000, 32'h7fffffff % 32'h80000000,
             32'h7fffffff ** 32'h80000000, 32'sh7fffffff / 32'h80000000, 32'sh7fffffff ** 32'h80000000, 32'h7fffffff ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 5,
             32'h7fffffff << 32'h80000000, 32'h7fffffff >> 32'h80000000, 32'sh7fffffff >>> 32'h80000000, 32'sh7fffffff >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 6,
             -32'sh7fffffff, 32'sh7fffffff + 32'sh7fffffff, 32'sh7fffffff - 32'sh7fffffff, 32'sh7fffffff * 32'sh7fffffff, 32'sh7fffffff / 32'sh7fffffff,
             32'sh7fffffff % 32'sh7fffffff, 32'sh7fffffff ** 32'sh7fffffff, 32'h7fffffff * 32'h7fffffff, 32'h7fffffff / 32'h7fffffff, 32'h7fffffff % 32'h7fffffff,
             32'h7fffffff ** 32'h7fffffff, 32'sh7fffffff / 32'h7fffffff, 32'sh7fffffff ** 32'h7fffffff, 32'h7fffffff ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 6,
             32'h7fffffff << 32'h7fffffff, 32'h7fffffff >> 32'h7fffffff, 32'sh7fffffff >>> 32'h7fffffff, 32'sh7fffffff >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 10,
             -32'sh7fffffff, 32'sh7fffffff + 32'shfffffffd, 32'sh7fffffff - 32'shfffffffd, 32'sh7fffffff * 32'shfffffffd, 32'sh7fffffff / 32'shfffffffd,
             32'sh7fffffff % 32'shfffffffd, 32'sh7fffffff ** 32'shfffffffd, 32'h7fffffff * 32'hfffffffd, 32'h7fffffff / 32'hfffffffd, 32'h7fffffff % 32'hfffffffd,
             32'h7fffffff ** 32'hfffffffd, 32'sh7fffffff / 32'hfffffffd, 32'sh7fffffff ** 32'hfffffffd, 32'h7fffffff ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 10,
             32'h7fffffff << 32'hfffffffd, 32'h7fffffff >> 32'hfffffffd, 32'sh7fffffff >>> 32'hfffffffd, 32'sh7fffffff >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 6, 11,
             -32'sh7fffffff, 32'sh7fffffff + 32'shx, 32'sh7fffffff - 32'shx, 32'sh7fffffff * 32'shx, 32'sh7fffffff / 32'shx,
             32'sh7fffffff % 32'shx, 32'sh7fffffff ** 32'shx, 32'h7fffffff * 32'hx, 32'h7fffffff / 32'hx, 32'h7fffffff % 32'hx,
             32'h7fffffff ** 32'hx, 32'sh7fffffff / 32'hx, 32'sh7fffffff ** 32'hx, 32'h7fffffff ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 6, 11,
             32'h7fffffff << 32'hx, 32'h7fffffff >> 32'hx, 32'sh7fffffff >>> 32'hx, 32'sh7fffffff >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 0,
             -32'shfffffffd, 32'shfffffffd + 32'sh0, 32'shfffffffd - 32'sh0, 32'shfffffffd * 32'sh0, 32'shfffffffd / 32'sh0,
             32'shfffffffd % 32'sh0, 32'shfffffffd ** 32'sh0, 32'hfffffffd * 32'h0, 32'hfffffffd / 32'h0, 32'hfffffffd % 32'h0,
             32'hfffffffd ** 32'h0, 32'shfffffffd / 32'h0, 32'shfffffffd ** 32'h0, 32'hfffffffd ** 32'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 0,
             32'hfffffffd << 32'h0, 32'hfffffffd >> 32'h0, 32'shfffffffd >>> 32'h0, 32'shfffffffd >>> 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 1,
             -32'shfffffffd, 32'shfffffffd + 32'sh1, 32'shfffffffd - 32'sh1, 32'shfffffffd * 32'sh1, 32'shfffffffd / 32'sh1,
             32'shfffffffd % 32'sh1, 32'shfffffffd ** 32'sh1, 32'hfffffffd * 32'h1, 32'hfffffffd / 32'h1, 32'hfffffffd % 32'h1,
             32'hfffffffd ** 32'h1, 32'shfffffffd / 32'h1, 32'shfffffffd ** 32'h1, 32'hfffffffd ** 32'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 1,
             32'hfffffffd << 32'h1, 32'hfffffffd >> 32'h1, 32'shfffffffd >>> 32'h1, 32'shfffffffd >>> 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 4,
             -32'shfffffffd, 32'shfffffffd + 32'shffffffff, 32'shfffffffd - 32'shffffffff, 32'shfffffffd * 32'shffffffff, 32'shfffffffd / 32'shffffffff,
             32'shfffffffd % 32'shffffffff, 32'shfffffffd ** 32'shffffffff, 32'hfffffffd * 32'hffffffff, 32'hfffffffd / 32'hffffffff, 32'hfffffffd % 32'hffffffff,
             32'hfffffffd ** 32'hffffffff, 32'shfffffffd / 32'hffffffff, 32'shfffffffd ** 32'hffffffff, 32'hfffffffd ** 32'shffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 4,
             32'hfffffffd << 32'hffffffff, 32'hfffffffd >> 32'hffffffff, 32'shfffffffd >>> 32'hffffffff, 32'shfffffffd >>> 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 5,
             -32'shfffffffd, 32'shfffffffd + 32'sh80000000, 32'shfffffffd - 32'sh80000000, 32'shfffffffd * 32'sh80000000, 32'shfffffffd / 32'sh80000000,
             32'shfffffffd % 32'sh80000000, 32'shfffffffd ** 32'sh80000000, 32'hfffffffd * 32'h80000000, 32'hfffffffd / 32'h80000000, 32'hfffffffd % 32'h80000000,
             32'hfffffffd ** 32'h80000000, 32'shfffffffd / 32'h80000000, 32'shfffffffd ** 32'h80000000, 32'hfffffffd ** 32'sh80000000);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 5,
             32'hfffffffd << 32'h80000000, 32'hfffffffd >> 32'h80000000, 32'shfffffffd >>> 32'h80000000, 32'shfffffffd >>> 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 6,
             -32'shfffffffd, 32'shfffffffd + 32'sh7fffffff, 32'shfffffffd - 32'sh7fffffff, 32'shfffffffd * 32'sh7fffffff, 32'shfffffffd / 32'sh7fffffff,
             32'shfffffffd % 32'sh7fffffff, 32'shfffffffd ** 32'sh7fffffff, 32'hfffffffd * 32'h7fffffff, 32'hfffffffd / 32'h7fffffff, 32'hfffffffd % 32'h7fffffff,
             32'hfffffffd ** 32'h7fffffff, 32'shfffffffd / 32'h7fffffff, 32'shfffffffd ** 32'h7fffffff, 32'hfffffffd ** 32'sh7fffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 6,
             32'hfffffffd << 32'h7fffffff, 32'hfffffffd >> 32'h7fffffff, 32'shfffffffd >>> 32'h7fffffff, 32'shfffffffd >>> 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 10,
             -32'shfffffffd, 32'shfffffffd + 32'shfffffffd, 32'shfffffffd - 32'shfffffffd, 32'shfffffffd * 32'shfffffffd, 32'shfffffffd / 32'shfffffffd,
             32'shfffffffd % 32'shfffffffd, 32'shfffffffd ** 32'shfffffffd, 32'hfffffffd * 32'hfffffffd, 32'hfffffffd / 32'hfffffffd, 32'hfffffffd % 32'hfffffffd,
             32'hfffffffd ** 32'hfffffffd, 32'shfffffffd / 32'hfffffffd, 32'shfffffffd ** 32'hfffffffd, 32'hfffffffd ** 32'shfffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 10,
             32'hfffffffd << 32'hfffffffd, 32'hfffffffd >> 32'hfffffffd, 32'shfffffffd >>> 32'hfffffffd, 32'shfffffffd >>> 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 10, 11,
             -32'shfffffffd, 32'shfffffffd + 32'shx, 32'shfffffffd - 32'shx, 32'shfffffffd * 32'shx, 32'shfffffffd / 32'shx,
             32'shfffffffd % 32'shx, 32'shfffffffd ** 32'shx, 32'hfffffffd * 32'hx, 32'hfffffffd / 32'hx, 32'hfffffffd % 32'hx,
             32'hfffffffd ** 32'hx, 32'shfffffffd / 32'hx, 32'shfffffffd ** 32'hx, 32'hfffffffd ** 32'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 32, 10, 11,
             32'hfffffffd << 32'hx, 32'hfffffffd >> 32'hx, 32'shfffffffd >>> 32'hx, 32'shfffffffd >>> 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 0,
             -32'shx, 32'shx + 32'sh0, 32'shx - 32'sh0, 32'shx * 32'sh0, 32'shx / 32'sh0,
             32'shx % 32'sh0, 32'shx ** 32'sh0, 32'hx * 32'h0, 32'hx / 32'h0, 32'hx % 32'h0,
             32'hx ** 32'h0, 32'shx / 32'h0, 32'shx ** 32'h0, 32'hx ** 32'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 1,
             -32'shx, 32'shx + 32'sh1, 32'shx - 32'sh1, 32'shx * 32'sh1, 32'shx / 32'sh1,
             32'shx % 32'sh1, 32'shx ** 32'sh1, 32'hx * 32'h1, 32'hx / 32'h1, 32'hx % 32'h1,
             32'hx ** 32'h1, 32'shx / 32'h1, 32'shx ** 32'h1, 32'hx ** 32'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 4,
             -32'shx, 32'shx + 32'shffffffff, 32'shx - 32'shffffffff, 32'shx * 32'shffffffff, 32'shx / 32'shffffffff,
             32'shx % 32'shffffffff, 32'shx ** 32'shffffffff, 32'hx * 32'hffffffff, 32'hx / 32'hffffffff, 32'hx % 32'hffffffff,
             32'hx ** 32'hffffffff, 32'shx / 32'hffffffff, 32'shx ** 32'hffffffff, 32'hx ** 32'shffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 5,
             -32'shx, 32'shx + 32'sh80000000, 32'shx - 32'sh80000000, 32'shx * 32'sh80000000, 32'shx / 32'sh80000000,
             32'shx % 32'sh80000000, 32'shx ** 32'sh80000000, 32'hx * 32'h80000000, 32'hx / 32'h80000000, 32'hx % 32'h80000000,
             32'hx ** 32'h80000000, 32'shx / 32'h80000000, 32'shx ** 32'h80000000, 32'hx ** 32'sh80000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 6,
             -32'shx, 32'shx + 32'sh7fffffff, 32'shx - 32'sh7fffffff, 32'shx * 32'sh7fffffff, 32'shx / 32'sh7fffffff,
             32'shx % 32'sh7fffffff, 32'shx ** 32'sh7fffffff, 32'hx * 32'h7fffffff, 32'hx / 32'h7fffffff, 32'hx % 32'h7fffffff,
             32'hx ** 32'h7fffffff, 32'shx / 32'h7fffffff, 32'shx ** 32'h7fffffff, 32'hx ** 32'sh7fffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 10,
             -32'shx, 32'shx + 32'shfffffffd, 32'shx - 32'shfffffffd, 32'shx * 32'shfffffffd, 32'shx / 32'shfffffffd,
             32'shx % 32'shfffffffd, 32'shx ** 32'shfffffffd, 32'hx * 32'hfffffffd, 32'hx / 32'hfffffffd, 32'hx % 32'hfffffffd,
             32'hx ** 32'hfffffffd, 32'shx / 32'hfffffffd, 32'shx ** 32'hfffffffd, 32'hx ** 32'shfffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 32, 11, 11,
             -32'shx, 32'shx + 32'shx, 32'shx - 32'shx, 32'shx * 32'shx, 32'shx / 32'shx,
             32'shx % 32'shx, 32'shx ** 32'shx, 32'hx * 32'hx, 32'hx / 32'hx, 32'hx % 32'hx,
             32'hx ** 32'hx, 32'shx / 32'hx, 32'shx ** 32'hx, 32'hx ** 32'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 0,
             -64'sh0, 64'sh0 + 64'sh0, 64'sh0 - 64'sh0, 64'sh0 * 64'sh0, 64'sh0 / 64'sh0,
             64'sh0 % 64'sh0, 64'sh0 ** 64'sh0, 64'h0 * 64'h0, 64'h0 / 64'h0, 64'h0 % 64'h0,
             64'h0 ** 64'h0, 64'sh0 / 64'h0, 64'sh0 ** 64'h0, 64'h0 ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 0,
             64'h0 << 64'h0, 64'h0 >> 64'h0, 64'sh0 >>> 64'h0, 64'sh0 >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 1,
             -64'sh0, 64'sh0 + 64'sh1, 64'sh0 - 64'sh1, 64'sh0 * 64'sh1, 64'sh0 / 64'sh1,
             64'sh0 % 64'sh1, 64'sh0 ** 64'sh1, 64'h0 * 64'h1, 64'h0 / 64'h1, 64'h0 % 64'h1,
             64'h0 ** 64'h1, 64'sh0 / 64'h1, 64'sh0 ** 64'h1, 64'h0 ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 1,
             64'h0 << 64'h1, 64'h0 >> 64'h1, 64'sh0 >>> 64'h1, 64'sh0 >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 4,
             -64'sh0, 64'sh0 + 64'shffffffffffffffff, 64'sh0 - 64'shffffffffffffffff, 64'sh0 * 64'shffffffffffffffff, 64'sh0 / 64'shffffffffffffffff,
             64'sh0 % 64'shffffffffffffffff, 64'sh0 ** 64'shffffffffffffffff, 64'h0 * 64'hffffffffffffffff, 64'h0 / 64'hffffffffffffffff, 64'h0 % 64'hffffffffffffffff,
             64'h0 ** 64'hffffffffffffffff, 64'sh0 / 64'hffffffffffffffff, 64'sh0 ** 64'hffffffffffffffff, 64'h0 ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 4,
             64'h0 << 64'hffffffffffffffff, 64'h0 >> 64'hffffffffffffffff, 64'sh0 >>> 64'hffffffffffffffff, 64'sh0 >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 5,
             -64'sh0, 64'sh0 + 64'sh8000000000000000, 64'sh0 - 64'sh8000000000000000, 64'sh0 * 64'sh8000000000000000, 64'sh0 / 64'sh8000000000000000,
             64'sh0 % 64'sh8000000000000000, 64'sh0 ** 64'sh8000000000000000, 64'h0 * 64'h8000000000000000, 64'h0 / 64'h8000000000000000, 64'h0 % 64'h8000000000000000,
             64'h0 ** 64'h8000000000000000, 64'sh0 / 64'h8000000000000000, 64'sh0 ** 64'h8000000000000000, 64'h0 ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 5,
             64'h0 << 64'h8000000000000000, 64'h0 >> 64'h8000000000000000, 64'sh0 >>> 64'h8000000000000000, 64'sh0 >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 6,
             -64'sh0, 64'sh0 + 64'sh7fffffffffffffff, 64'sh0 - 64'sh7fffffffffffffff, 64'sh0 * 64'sh7fffffffffffffff, 64'sh0 / 64'sh7fffffffffffffff,
             64'sh0 % 64'sh7fffffffffffffff, 64'sh0 ** 64'sh7fffffffffffffff, 64'h0 * 64'h7fffffffffffffff, 64'h0 / 64'h7fffffffffffffff, 64'h0 % 64'h7fffffffffffffff,
             64'h0 ** 64'h7fffffffffffffff, 64'sh0 / 64'h7fffffffffffffff, 64'sh0 ** 64'h7fffffffffffffff, 64'h0 ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 6,
             64'h0 << 64'h7fffffffffffffff, 64'h0 >> 64'h7fffffffffffffff, 64'sh0 >>> 64'h7fffffffffffffff, 64'sh0 >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 10,
             -64'sh0, 64'sh0 + 64'shfffffffffffffffd, 64'sh0 - 64'shfffffffffffffffd, 64'sh0 * 64'shfffffffffffffffd, 64'sh0 / 64'shfffffffffffffffd,
             64'sh0 % 64'shfffffffffffffffd, 64'sh0 ** 64'shfffffffffffffffd, 64'h0 * 64'hfffffffffffffffd, 64'h0 / 64'hfffffffffffffffd, 64'h0 % 64'hfffffffffffffffd,
             64'h0 ** 64'hfffffffffffffffd, 64'sh0 / 64'hfffffffffffffffd, 64'sh0 ** 64'hfffffffffffffffd, 64'h0 ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 10,
             64'h0 << 64'hfffffffffffffffd, 64'h0 >> 64'hfffffffffffffffd, 64'sh0 >>> 64'hfffffffffffffffd, 64'sh0 >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 0, 11,
             -64'sh0, 64'sh0 + 64'shx, 64'sh0 - 64'shx, 64'sh0 * 64'shx, 64'sh0 / 64'shx,
             64'sh0 % 64'shx, 64'sh0 ** 64'shx, 64'h0 * 64'hx, 64'h0 / 64'hx, 64'h0 % 64'hx,
             64'h0 ** 64'hx, 64'sh0 / 64'hx, 64'sh0 ** 64'hx, 64'h0 ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 0, 11,
             64'h0 << 64'hx, 64'h0 >> 64'hx, 64'sh0 >>> 64'hx, 64'sh0 >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 0,
             -64'sh1, 64'sh1 + 64'sh0, 64'sh1 - 64'sh0, 64'sh1 * 64'sh0, 64'sh1 / 64'sh0,
             64'sh1 % 64'sh0, 64'sh1 ** 64'sh0, 64'h1 * 64'h0, 64'h1 / 64'h0, 64'h1 % 64'h0,
             64'h1 ** 64'h0, 64'sh1 / 64'h0, 64'sh1 ** 64'h0, 64'h1 ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 0,
             64'h1 << 64'h0, 64'h1 >> 64'h0, 64'sh1 >>> 64'h0, 64'sh1 >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 1,
             -64'sh1, 64'sh1 + 64'sh1, 64'sh1 - 64'sh1, 64'sh1 * 64'sh1, 64'sh1 / 64'sh1,
             64'sh1 % 64'sh1, 64'sh1 ** 64'sh1, 64'h1 * 64'h1, 64'h1 / 64'h1, 64'h1 % 64'h1,
             64'h1 ** 64'h1, 64'sh1 / 64'h1, 64'sh1 ** 64'h1, 64'h1 ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 1,
             64'h1 << 64'h1, 64'h1 >> 64'h1, 64'sh1 >>> 64'h1, 64'sh1 >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 4,
             -64'sh1, 64'sh1 + 64'shffffffffffffffff, 64'sh1 - 64'shffffffffffffffff, 64'sh1 * 64'shffffffffffffffff, 64'sh1 / 64'shffffffffffffffff,
             64'sh1 % 64'shffffffffffffffff, 64'sh1 ** 64'shffffffffffffffff, 64'h1 * 64'hffffffffffffffff, 64'h1 / 64'hffffffffffffffff, 64'h1 % 64'hffffffffffffffff,
             64'h1 ** 64'hffffffffffffffff, 64'sh1 / 64'hffffffffffffffff, 64'sh1 ** 64'hffffffffffffffff, 64'h1 ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 4,
             64'h1 << 64'hffffffffffffffff, 64'h1 >> 64'hffffffffffffffff, 64'sh1 >>> 64'hffffffffffffffff, 64'sh1 >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 5,
             -64'sh1, 64'sh1 + 64'sh8000000000000000, 64'sh1 - 64'sh8000000000000000, 64'sh1 * 64'sh8000000000000000, 64'sh1 / 64'sh8000000000000000,
             64'sh1 % 64'sh8000000000000000, 64'sh1 ** 64'sh8000000000000000, 64'h1 * 64'h8000000000000000, 64'h1 / 64'h8000000000000000, 64'h1 % 64'h8000000000000000,
             64'h1 ** 64'h8000000000000000, 64'sh1 / 64'h8000000000000000, 64'sh1 ** 64'h8000000000000000, 64'h1 ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 5,
             64'h1 << 64'h8000000000000000, 64'h1 >> 64'h8000000000000000, 64'sh1 >>> 64'h8000000000000000, 64'sh1 >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 6,
             -64'sh1, 64'sh1 + 64'sh7fffffffffffffff, 64'sh1 - 64'sh7fffffffffffffff, 64'sh1 * 64'sh7fffffffffffffff, 64'sh1 / 64'sh7fffffffffffffff,
             64'sh1 % 64'sh7fffffffffffffff, 64'sh1 ** 64'sh7fffffffffffffff, 64'h1 * 64'h7fffffffffffffff, 64'h1 / 64'h7fffffffffffffff, 64'h1 % 64'h7fffffffffffffff,
             64'h1 ** 64'h7fffffffffffffff, 64'sh1 / 64'h7fffffffffffffff, 64'sh1 ** 64'h7fffffffffffffff, 64'h1 ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 6,
             64'h1 << 64'h7fffffffffffffff, 64'h1 >> 64'h7fffffffffffffff, 64'sh1 >>> 64'h7fffffffffffffff, 64'sh1 >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 10,
             -64'sh1, 64'sh1 + 64'shfffffffffffffffd, 64'sh1 - 64'shfffffffffffffffd, 64'sh1 * 64'shfffffffffffffffd, 64'sh1 / 64'shfffffffffffffffd,
             64'sh1 % 64'shfffffffffffffffd, 64'sh1 ** 64'shfffffffffffffffd, 64'h1 * 64'hfffffffffffffffd, 64'h1 / 64'hfffffffffffffffd, 64'h1 % 64'hfffffffffffffffd,
             64'h1 ** 64'hfffffffffffffffd, 64'sh1 / 64'hfffffffffffffffd, 64'sh1 ** 64'hfffffffffffffffd, 64'h1 ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 10,
             64'h1 << 64'hfffffffffffffffd, 64'h1 >> 64'hfffffffffffffffd, 64'sh1 >>> 64'hfffffffffffffffd, 64'sh1 >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 1, 11,
             -64'sh1, 64'sh1 + 64'shx, 64'sh1 - 64'shx, 64'sh1 * 64'shx, 64'sh1 / 64'shx,
             64'sh1 % 64'shx, 64'sh1 ** 64'shx, 64'h1 * 64'hx, 64'h1 / 64'hx, 64'h1 % 64'hx,
             64'h1 ** 64'hx, 64'sh1 / 64'hx, 64'sh1 ** 64'hx, 64'h1 ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 1, 11,
             64'h1 << 64'hx, 64'h1 >> 64'hx, 64'sh1 >>> 64'hx, 64'sh1 >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 0,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'sh0, 64'shffffffffffffffff - 64'sh0, 64'shffffffffffffffff * 64'sh0, 64'shffffffffffffffff / 64'sh0,
             64'shffffffffffffffff % 64'sh0, 64'shffffffffffffffff ** 64'sh0, 64'hffffffffffffffff * 64'h0, 64'hffffffffffffffff / 64'h0, 64'hffffffffffffffff % 64'h0,
             64'hffffffffffffffff ** 64'h0, 64'shffffffffffffffff / 64'h0, 64'shffffffffffffffff ** 64'h0, 64'hffffffffffffffff ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 0,
             64'hffffffffffffffff << 64'h0, 64'hffffffffffffffff >> 64'h0, 64'shffffffffffffffff >>> 64'h0, 64'shffffffffffffffff >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 1,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'sh1, 64'shffffffffffffffff - 64'sh1, 64'shffffffffffffffff * 64'sh1, 64'shffffffffffffffff / 64'sh1,
             64'shffffffffffffffff % 64'sh1, 64'shffffffffffffffff ** 64'sh1, 64'hffffffffffffffff * 64'h1, 64'hffffffffffffffff / 64'h1, 64'hffffffffffffffff % 64'h1,
             64'hffffffffffffffff ** 64'h1, 64'shffffffffffffffff / 64'h1, 64'shffffffffffffffff ** 64'h1, 64'hffffffffffffffff ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 1,
             64'hffffffffffffffff << 64'h1, 64'hffffffffffffffff >> 64'h1, 64'shffffffffffffffff >>> 64'h1, 64'shffffffffffffffff >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 4,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'shffffffffffffffff, 64'shffffffffffffffff - 64'shffffffffffffffff, 64'shffffffffffffffff * 64'shffffffffffffffff, 64'shffffffffffffffff / 64'shffffffffffffffff,
             64'shffffffffffffffff % 64'shffffffffffffffff, 64'shffffffffffffffff ** 64'shffffffffffffffff, 64'hffffffffffffffff * 64'hffffffffffffffff, 64'hffffffffffffffff / 64'hffffffffffffffff, 64'hffffffffffffffff % 64'hffffffffffffffff,
             64'hffffffffffffffff ** 64'hffffffffffffffff, 64'shffffffffffffffff / 64'hffffffffffffffff, 64'shffffffffffffffff ** 64'hffffffffffffffff, 64'hffffffffffffffff ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 4,
             64'hffffffffffffffff << 64'hffffffffffffffff, 64'hffffffffffffffff >> 64'hffffffffffffffff, 64'shffffffffffffffff >>> 64'hffffffffffffffff, 64'shffffffffffffffff >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 5,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'sh8000000000000000, 64'shffffffffffffffff - 64'sh8000000000000000, 64'shffffffffffffffff * 64'sh8000000000000000, 64'shffffffffffffffff / 64'sh8000000000000000,
             64'shffffffffffffffff % 64'sh8000000000000000, 64'shffffffffffffffff ** 64'sh8000000000000000, 64'hffffffffffffffff * 64'h8000000000000000, 64'hffffffffffffffff / 64'h8000000000000000, 64'hffffffffffffffff % 64'h8000000000000000,
             64'hffffffffffffffff ** 64'h8000000000000000, 64'shffffffffffffffff / 64'h8000000000000000, 64'shffffffffffffffff ** 64'h8000000000000000, 64'hffffffffffffffff ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 5,
             64'hffffffffffffffff << 64'h8000000000000000, 64'hffffffffffffffff >> 64'h8000000000000000, 64'shffffffffffffffff >>> 64'h8000000000000000, 64'shffffffffffffffff >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 6,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'sh7fffffffffffffff, 64'shffffffffffffffff - 64'sh7fffffffffffffff, 64'shffffffffffffffff * 64'sh7fffffffffffffff, 64'shffffffffffffffff / 64'sh7fffffffffffffff,
             64'shffffffffffffffff % 64'sh7fffffffffffffff, 64'shffffffffffffffff ** 64'sh7fffffffffffffff, 64'hffffffffffffffff * 64'h7fffffffffffffff, 64'hffffffffffffffff / 64'h7fffffffffffffff, 64'hffffffffffffffff % 64'h7fffffffffffffff,
             64'hffffffffffffffff ** 64'h7fffffffffffffff, 64'shffffffffffffffff / 64'h7fffffffffffffff, 64'shffffffffffffffff ** 64'h7fffffffffffffff, 64'hffffffffffffffff ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 6,
             64'hffffffffffffffff << 64'h7fffffffffffffff, 64'hffffffffffffffff >> 64'h7fffffffffffffff, 64'shffffffffffffffff >>> 64'h7fffffffffffffff, 64'shffffffffffffffff >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 10,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'shfffffffffffffffd, 64'shffffffffffffffff - 64'shfffffffffffffffd, 64'shffffffffffffffff * 64'shfffffffffffffffd, 64'shffffffffffffffff / 64'shfffffffffffffffd,
             64'shffffffffffffffff % 64'shfffffffffffffffd, 64'shffffffffffffffff ** 64'shfffffffffffffffd, 64'hffffffffffffffff * 64'hfffffffffffffffd, 64'hffffffffffffffff / 64'hfffffffffffffffd, 64'hffffffffffffffff % 64'hfffffffffffffffd,
             64'hffffffffffffffff ** 64'hfffffffffffffffd, 64'shffffffffffffffff / 64'hfffffffffffffffd, 64'shffffffffffffffff ** 64'hfffffffffffffffd, 64'hffffffffffffffff ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 10,
             64'hffffffffffffffff << 64'hfffffffffffffffd, 64'hffffffffffffffff >> 64'hfffffffffffffffd, 64'shffffffffffffffff >>> 64'hfffffffffffffffd, 64'shffffffffffffffff >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 4, 11,
             -64'shffffffffffffffff, 64'shffffffffffffffff + 64'shx, 64'shffffffffffffffff - 64'shx, 64'shffffffffffffffff * 64'shx, 64'shffffffffffffffff / 64'shx,
             64'shffffffffffffffff % 64'shx, 64'shffffffffffffffff ** 64'shx, 64'hffffffffffffffff * 64'hx, 64'hffffffffffffffff / 64'hx, 64'hffffffffffffffff % 64'hx,
             64'hffffffffffffffff ** 64'hx, 64'shffffffffffffffff / 64'hx, 64'shffffffffffffffff ** 64'hx, 64'hffffffffffffffff ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 4, 11,
             64'hffffffffffffffff << 64'hx, 64'hffffffffffffffff >> 64'hx, 64'shffffffffffffffff >>> 64'hx, 64'shffffffffffffffff >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 0,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'sh0, 64'sh8000000000000000 - 64'sh0, 64'sh8000000000000000 * 64'sh0, 64'sh8000000000000000 / 64'sh0,
             64'sh8000000000000000 % 64'sh0, 64'sh8000000000000000 ** 64'sh0, 64'h8000000000000000 * 64'h0, 64'h8000000000000000 / 64'h0, 64'h8000000000000000 % 64'h0,
             64'h8000000000000000 ** 64'h0, 64'sh8000000000000000 / 64'h0, 64'sh8000000000000000 ** 64'h0, 64'h8000000000000000 ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 0,
             64'h8000000000000000 << 64'h0, 64'h8000000000000000 >> 64'h0, 64'sh8000000000000000 >>> 64'h0, 64'sh8000000000000000 >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 1,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'sh1, 64'sh8000000000000000 - 64'sh1, 64'sh8000000000000000 * 64'sh1, 64'sh8000000000000000 / 64'sh1,
             64'sh8000000000000000 % 64'sh1, 64'sh8000000000000000 ** 64'sh1, 64'h8000000000000000 * 64'h1, 64'h8000000000000000 / 64'h1, 64'h8000000000000000 % 64'h1,
             64'h8000000000000000 ** 64'h1, 64'sh8000000000000000 / 64'h1, 64'sh8000000000000000 ** 64'h1, 64'h8000000000000000 ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 1,
             64'h8000000000000000 << 64'h1, 64'h8000000000000000 >> 64'h1, 64'sh8000000000000000 >>> 64'h1, 64'sh8000000000000000 >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 4,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'shffffffffffffffff, 64'sh8000000000000000 - 64'shffffffffffffffff, 64'sh8000000000000000 * 64'shffffffffffffffff, 64'sh8000000000000000 / 64'shffffffffffffffff,
             64'sh8000000000000000 % 64'shffffffffffffffff, 64'sh8000000000000000 ** 64'shffffffffffffffff, 64'h8000000000000000 * 64'hffffffffffffffff, 64'h8000000000000000 / 64'hffffffffffffffff, 64'h8000000000000000 % 64'hffffffffffffffff,
             64'h8000000000000000 ** 64'hffffffffffffffff, 64'sh8000000000000000 / 64'hffffffffffffffff, 64'sh8000000000000000 ** 64'hffffffffffffffff, 64'h8000000000000000 ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 4,
             64'h8000000000000000 << 64'hffffffffffffffff, 64'h8000000000000000 >> 64'hffffffffffffffff, 64'sh8000000000000000 >>> 64'hffffffffffffffff, 64'sh8000000000000000 >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 5,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'sh8000000000000000, 64'sh8000000000000000 - 64'sh8000000000000000, 64'sh8000000000000000 * 64'sh8000000000000000, 64'sh8000000000000000 / 64'sh8000000000000000,
             64'sh8000000000000000 % 64'sh8000000000000000, 64'sh8000000000000000 ** 64'sh8000000000000000, 64'h8000000000000000 * 64'h8000000000000000, 64'h8000000000000000 / 64'h8000000000000000, 64'h8000000000000000 % 64'h8000000000000000,
             64'h8000000000000000 ** 64'h8000000000000000, 64'sh8000000000000000 / 64'h8000000000000000, 64'sh8000000000000000 ** 64'h8000000000000000, 64'h8000000000000000 ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 5,
             64'h8000000000000000 << 64'h8000000000000000, 64'h8000000000000000 >> 64'h8000000000000000, 64'sh8000000000000000 >>> 64'h8000000000000000, 64'sh8000000000000000 >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 6,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'sh7fffffffffffffff, 64'sh8000000000000000 - 64'sh7fffffffffffffff, 64'sh8000000000000000 * 64'sh7fffffffffffffff, 64'sh8000000000000000 / 64'sh7fffffffffffffff,
             64'sh8000000000000000 % 64'sh7fffffffffffffff, 64'sh8000000000000000 ** 64'sh7fffffffffffffff, 64'h8000000000000000 * 64'h7fffffffffffffff, 64'h8000000000000000 / 64'h7fffffffffffffff, 64'h8000000000000000 % 64'h7fffffffffffffff,
             64'h8000000000000000 ** 64'h7fffffffffffffff, 64'sh8000000000000000 / 64'h7fffffffffffffff, 64'sh8000000000000000 ** 64'h7fffffffffffffff, 64'h8000000000000000 ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 6,
             64'h8000000000000000 << 64'h7fffffffffffffff, 64'h8000000000000000 >> 64'h7fffffffffffffff, 64'sh8000000000000000 >>> 64'h7fffffffffffffff, 64'sh8000000000000000 >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 10,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'shfffffffffffffffd, 64'sh8000000000000000 - 64'shfffffffffffffffd, 64'sh8000000000000000 * 64'shfffffffffffffffd, 64'sh8000000000000000 / 64'shfffffffffffffffd,
             64'sh8000000000000000 % 64'shfffffffffffffffd, 64'sh8000000000000000 ** 64'shfffffffffffffffd, 64'h8000000000000000 * 64'hfffffffffffffffd, 64'h8000000000000000 / 64'hfffffffffffffffd, 64'h8000000000000000 % 64'hfffffffffffffffd,
             64'h8000000000000000 ** 64'hfffffffffffffffd, 64'sh8000000000000000 / 64'hfffffffffffffffd, 64'sh8000000000000000 ** 64'hfffffffffffffffd, 64'h8000000000000000 ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 10,
             64'h8000000000000000 << 64'hfffffffffffffffd, 64'h8000000000000000 >> 64'hfffffffffffffffd, 64'sh8000000000000000 >>> 64'hfffffffffffffffd, 64'sh8000000000000000 >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 5, 11,
             -64'sh8000000000000000, 64'sh8000000000000000 + 64'shx, 64'sh8000000000000000 - 64'shx, 64'sh8000000000000000 * 64'shx, 64'sh8000000000000000 / 64'shx,
             64'sh8000000000000000 % 64'shx, 64'sh8000000000000000 ** 64'shx, 64'h8000000000000000 * 64'hx, 64'h8000000000000000 / 64'hx, 64'h8000000000000000 % 64'hx,
             64'h8000000000000000 ** 64'hx, 64'sh8000000000000000 / 64'hx, 64'sh8000000000000000 ** 64'hx, 64'h8000000000000000 ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 5, 11,
             64'h8000000000000000 << 64'hx, 64'h8000000000000000 >> 64'hx, 64'sh8000000000000000 >>> 64'hx, 64'sh8000000000000000 >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 0,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'sh0, 64'sh7fffffffffffffff - 64'sh0, 64'sh7fffffffffffffff * 64'sh0, 64'sh7fffffffffffffff / 64'sh0,
             64'sh7fffffffffffffff % 64'sh0, 64'sh7fffffffffffffff ** 64'sh0, 64'h7fffffffffffffff * 64'h0, 64'h7fffffffffffffff / 64'h0, 64'h7fffffffffffffff % 64'h0,
             64'h7fffffffffffffff ** 64'h0, 64'sh7fffffffffffffff / 64'h0, 64'sh7fffffffffffffff ** 64'h0, 64'h7fffffffffffffff ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 0,
             64'h7fffffffffffffff << 64'h0, 64'h7fffffffffffffff >> 64'h0, 64'sh7fffffffffffffff >>> 64'h0, 64'sh7fffffffffffffff >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 1,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'sh1, 64'sh7fffffffffffffff - 64'sh1, 64'sh7fffffffffffffff * 64'sh1, 64'sh7fffffffffffffff / 64'sh1,
             64'sh7fffffffffffffff % 64'sh1, 64'sh7fffffffffffffff ** 64'sh1, 64'h7fffffffffffffff * 64'h1, 64'h7fffffffffffffff / 64'h1, 64'h7fffffffffffffff % 64'h1,
             64'h7fffffffffffffff ** 64'h1, 64'sh7fffffffffffffff / 64'h1, 64'sh7fffffffffffffff ** 64'h1, 64'h7fffffffffffffff ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 1,
             64'h7fffffffffffffff << 64'h1, 64'h7fffffffffffffff >> 64'h1, 64'sh7fffffffffffffff >>> 64'h1, 64'sh7fffffffffffffff >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 4,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'shffffffffffffffff, 64'sh7fffffffffffffff - 64'shffffffffffffffff, 64'sh7fffffffffffffff * 64'shffffffffffffffff, 64'sh7fffffffffffffff / 64'shffffffffffffffff,
             64'sh7fffffffffffffff % 64'shffffffffffffffff, 64'sh7fffffffffffffff ** 64'shffffffffffffffff, 64'h7fffffffffffffff * 64'hffffffffffffffff, 64'h7fffffffffffffff / 64'hffffffffffffffff, 64'h7fffffffffffffff % 64'hffffffffffffffff,
             64'h7fffffffffffffff ** 64'hffffffffffffffff, 64'sh7fffffffffffffff / 64'hffffffffffffffff, 64'sh7fffffffffffffff ** 64'hffffffffffffffff, 64'h7fffffffffffffff ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 4,
             64'h7fffffffffffffff << 64'hffffffffffffffff, 64'h7fffffffffffffff >> 64'hffffffffffffffff, 64'sh7fffffffffffffff >>> 64'hffffffffffffffff, 64'sh7fffffffffffffff >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 5,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'sh8000000000000000, 64'sh7fffffffffffffff - 64'sh8000000000000000, 64'sh7fffffffffffffff * 64'sh8000000000000000, 64'sh7fffffffffffffff / 64'sh8000000000000000,
             64'sh7fffffffffffffff % 64'sh8000000000000000, 64'sh7fffffffffffffff ** 64'sh8000000000000000, 64'h7fffffffffffffff * 64'h8000000000000000, 64'h7fffffffffffffff / 64'h8000000000000000, 64'h7fffffffffffffff % 64'h8000000000000000,
             64'h7fffffffffffffff ** 64'h8000000000000000, 64'sh7fffffffffffffff / 64'h8000000000000000, 64'sh7fffffffffffffff ** 64'h8000000000000000, 64'h7fffffffffffffff ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 5,
             64'h7fffffffffffffff << 64'h8000000000000000, 64'h7fffffffffffffff >> 64'h8000000000000000, 64'sh7fffffffffffffff >>> 64'h8000000000000000, 64'sh7fffffffffffffff >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 6,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'sh7fffffffffffffff, 64'sh7fffffffffffffff - 64'sh7fffffffffffffff, 64'sh7fffffffffffffff * 64'sh7fffffffffffffff, 64'sh7fffffffffffffff / 64'sh7fffffffffffffff,
             64'sh7fffffffffffffff % 64'sh7fffffffffffffff, 64'sh7fffffffffffffff ** 64'sh7fffffffffffffff, 64'h7fffffffffffffff * 64'h7fffffffffffffff, 64'h7fffffffffffffff / 64'h7fffffffffffffff, 64'h7fffffffffffffff % 64'h7fffffffffffffff,
             64'h7fffffffffffffff ** 64'h7fffffffffffffff, 64'sh7fffffffffffffff / 64'h7fffffffffffffff, 64'sh7fffffffffffffff ** 64'h7fffffffffffffff, 64'h7fffffffffffffff ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 6,
             64'h7fffffffffffffff << 64'h7fffffffffffffff, 64'h7fffffffffffffff >> 64'h7fffffffffffffff, 64'sh7fffffffffffffff >>> 64'h7fffffffffffffff, 64'sh7fffffffffffffff >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 10,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'shfffffffffffffffd, 64'sh7fffffffffffffff - 64'shfffffffffffffffd, 64'sh7fffffffffffffff * 64'shfffffffffffffffd, 64'sh7fffffffffffffff / 64'shfffffffffffffffd,
             64'sh7fffffffffffffff % 64'shfffffffffffffffd, 64'sh7fffffffffffffff ** 64'shfffffffffffffffd, 64'h7fffffffffffffff * 64'hfffffffffffffffd, 64'h7fffffffffffffff / 64'hfffffffffffffffd, 64'h7fffffffffffffff % 64'hfffffffffffffffd,
             64'h7fffffffffffffff ** 64'hfffffffffffffffd, 64'sh7fffffffffffffff / 64'hfffffffffffffffd, 64'sh7fffffffffffffff ** 64'hfffffffffffffffd, 64'h7fffffffffffffff ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 10,
             64'h7fffffffffffffff << 64'hfffffffffffffffd, 64'h7fffffffffffffff >> 64'hfffffffffffffffd, 64'sh7fffffffffffffff >>> 64'hfffffffffffffffd, 64'sh7fffffffffffffff >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 6, 11,
             -64'sh7fffffffffffffff, 64'sh7fffffffffffffff + 64'shx, 64'sh7fffffffffffffff - 64'shx, 64'sh7fffffffffffffff * 64'shx, 64'sh7fffffffffffffff / 64'shx,
             64'sh7fffffffffffffff % 64'shx, 64'sh7fffffffffffffff ** 64'shx, 64'h7fffffffffffffff * 64'hx, 64'h7fffffffffffffff / 64'hx, 64'h7fffffffffffffff % 64'hx,
             64'h7fffffffffffffff ** 64'hx, 64'sh7fffffffffffffff / 64'hx, 64'sh7fffffffffffffff ** 64'hx, 64'h7fffffffffffffff ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 6, 11,
             64'h7fffffffffffffff << 64'hx, 64'h7fffffffffffffff >> 64'hx, 64'sh7fffffffffffffff >>> 64'hx, 64'sh7fffffffffffffff >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 0,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'sh0, 64'shfffffffffffffffd - 64'sh0, 64'shfffffffffffffffd * 64'sh0, 64'shfffffffffffffffd / 64'sh0,
             64'shfffffffffffffffd % 64'sh0, 64'shfffffffffffffffd ** 64'sh0, 64'hfffffffffffffffd * 64'h0, 64'hfffffffffffffffd / 64'h0, 64'hfffffffffffffffd % 64'h0,
             64'hfffffffffffffffd ** 64'h0, 64'shfffffffffffffffd / 64'h0, 64'shfffffffffffffffd ** 64'h0, 64'hfffffffffffffffd ** 64'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 0,
             64'hfffffffffffffffd << 64'h0, 64'hfffffffffffffffd >> 64'h0, 64'shfffffffffffffffd >>> 64'h0, 64'shfffffffffffffffd >>> 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 1,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'sh1, 64'shfffffffffffffffd - 64'sh1, 64'shfffffffffffffffd * 64'sh1, 64'shfffffffffffffffd / 64'sh1,
             64'shfffffffffffffffd % 64'sh1, 64'shfffffffffffffffd ** 64'sh1, 64'hfffffffffffffffd * 64'h1, 64'hfffffffffffffffd / 64'h1, 64'hfffffffffffffffd % 64'h1,
             64'hfffffffffffffffd ** 64'h1, 64'shfffffffffffffffd / 64'h1, 64'shfffffffffffffffd ** 64'h1, 64'hfffffffffffffffd ** 64'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 1,
             64'hfffffffffffffffd << 64'h1, 64'hfffffffffffffffd >> 64'h1, 64'shfffffffffffffffd >>> 64'h1, 64'shfffffffffffffffd >>> 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 4,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'shffffffffffffffff, 64'shfffffffffffffffd - 64'shffffffffffffffff, 64'shfffffffffffffffd * 64'shffffffffffffffff, 64'shfffffffffffffffd / 64'shffffffffffffffff,
             64'shfffffffffffffffd % 64'shffffffffffffffff, 64'shfffffffffffffffd ** 64'shffffffffffffffff, 64'hfffffffffffffffd * 64'hffffffffffffffff, 64'hfffffffffffffffd / 64'hffffffffffffffff, 64'hfffffffffffffffd % 64'hffffffffffffffff,
             64'hfffffffffffffffd ** 64'hffffffffffffffff, 64'shfffffffffffffffd / 64'hffffffffffffffff, 64'shfffffffffffffffd ** 64'hffffffffffffffff, 64'hfffffffffffffffd ** 64'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 4,
             64'hfffffffffffffffd << 64'hffffffffffffffff, 64'hfffffffffffffffd >> 64'hffffffffffffffff, 64'shfffffffffffffffd >>> 64'hffffffffffffffff, 64'shfffffffffffffffd >>> 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 5,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'sh8000000000000000, 64'shfffffffffffffffd - 64'sh8000000000000000, 64'shfffffffffffffffd * 64'sh8000000000000000, 64'shfffffffffffffffd / 64'sh8000000000000000,
             64'shfffffffffffffffd % 64'sh8000000000000000, 64'shfffffffffffffffd ** 64'sh8000000000000000, 64'hfffffffffffffffd * 64'h8000000000000000, 64'hfffffffffffffffd / 64'h8000000000000000, 64'hfffffffffffffffd % 64'h8000000000000000,
             64'hfffffffffffffffd ** 64'h8000000000000000, 64'shfffffffffffffffd / 64'h8000000000000000, 64'shfffffffffffffffd ** 64'h8000000000000000, 64'hfffffffffffffffd ** 64'sh8000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 5,
             64'hfffffffffffffffd << 64'h8000000000000000, 64'hfffffffffffffffd >> 64'h8000000000000000, 64'shfffffffffffffffd >>> 64'h8000000000000000, 64'shfffffffffffffffd >>> 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 6,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'sh7fffffffffffffff, 64'shfffffffffffffffd - 64'sh7fffffffffffffff, 64'shfffffffffffffffd * 64'sh7fffffffffffffff, 64'shfffffffffffffffd / 64'sh7fffffffffffffff,
             64'shfffffffffffffffd % 64'sh7fffffffffffffff, 64'shfffffffffffffffd ** 64'sh7fffffffffffffff, 64'hfffffffffffffffd * 64'h7fffffffffffffff, 64'hfffffffffffffffd / 64'h7fffffffffffffff, 64'hfffffffffffffffd % 64'h7fffffffffffffff,
             64'hfffffffffffffffd ** 64'h7fffffffffffffff, 64'shfffffffffffffffd / 64'h7fffffffffffffff, 64'shfffffffffffffffd ** 64'h7fffffffffffffff, 64'hfffffffffffffffd ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 6,
             64'hfffffffffffffffd << 64'h7fffffffffffffff, 64'hfffffffffffffffd >> 64'h7fffffffffffffff, 64'shfffffffffffffffd >>> 64'h7fffffffffffffff, 64'shfffffffffffffffd >>> 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 10,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'shfffffffffffffffd, 64'shfffffffffffffffd - 64'shfffffffffffffffd, 64'shfffffffffffffffd * 64'shfffffffffffffffd, 64'shfffffffffffffffd / 64'shfffffffffffffffd,
             64'shfffffffffffffffd % 64'shfffffffffffffffd, 64'shfffffffffffffffd ** 64'shfffffffffffffffd, 64'hfffffffffffffffd * 64'hfffffffffffffffd, 64'hfffffffffffffffd / 64'hfffffffffffffffd, 64'hfffffffffffffffd % 64'hfffffffffffffffd,
             64'hfffffffffffffffd ** 64'hfffffffffffffffd, 64'shfffffffffffffffd / 64'hfffffffffffffffd, 64'shfffffffffffffffd ** 64'hfffffffffffffffd, 64'hfffffffffffffffd ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 10,
             64'hfffffffffffffffd << 64'hfffffffffffffffd, 64'hfffffffffffffffd >> 64'hfffffffffffffffd, 64'shfffffffffffffffd >>> 64'hfffffffffffffffd, 64'shfffffffffffffffd >>> 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 10, 11,
             -64'shfffffffffffffffd, 64'shfffffffffffffffd + 64'shx, 64'shfffffffffffffffd - 64'shx, 64'shfffffffffffffffd * 64'shx, 64'shfffffffffffffffd / 64'shx,
             64'shfffffffffffffffd % 64'shx, 64'shfffffffffffffffd ** 64'shx, 64'hfffffffffffffffd * 64'hx, 64'hfffffffffffffffd / 64'hx, 64'hfffffffffffffffd % 64'hx,
             64'hfffffffffffffffd ** 64'hx, 64'shfffffffffffffffd / 64'hx, 64'shfffffffffffffffd ** 64'hx, 64'hfffffffffffffffd ** 64'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 64, 10, 11,
             64'hfffffffffffffffd << 64'hx, 64'hfffffffffffffffd >> 64'hx, 64'shfffffffffffffffd >>> 64'hx, 64'shfffffffffffffffd >>> 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 0,
             -64'shx, 64'shx + 64'sh0, 64'shx - 64'sh0, 64'shx * 64'sh0, 64'shx / 64'sh0,
             64'shx % 64'sh0, 64'shx ** 64'sh0, 64'hx * 64'h0, 64'hx / 64'h0, 64'hx % 64'h0,
             64'hx ** 64'h0, 64'shx / 64'h0, 64'shx ** 64'h0, 64'hx ** 64'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 1,
             -64'shx, 64'shx + 64'sh1, 64'shx - 64'sh1, 64'shx * 64'sh1, 64'shx / 64'sh1,
             64'shx % 64'sh1, 64'shx ** 64'sh1, 64'hx * 64'h1, 64'hx / 64'h1, 64'hx % 64'h1,
             64'hx ** 64'h1, 64'shx / 64'h1, 64'shx ** 64'h1, 64'hx ** 64'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 4,
             -64'shx, 64'shx + 64'shffffffffffffffff, 64'shx - 64'shffffffffffffffff, 64'shx * 64'shffffffffffffffff, 64'shx / 64'shffffffffffffffff,
             64'shx % 64'shffffffffffffffff, 64'shx ** 64'shffffffffffffffff, 64'hx * 64'hffffffffffffffff, 64'hx / 64'hffffffffffffffff, 64'hx % 64'hffffffffffffffff,
             64'hx ** 64'hffffffffffffffff, 64'shx / 64'hffffffffffffffff, 64'shx ** 64'hffffffffffffffff, 64'hx ** 64'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 5,
             -64'shx, 64'shx + 64'sh8000000000000000, 64'shx - 64'sh8000000000000000, 64'shx * 64'sh8000000000000000, 64'shx / 64'sh8000000000000000,
             64'shx % 64'sh8000000000000000, 64'shx ** 64'sh8000000000000000, 64'hx * 64'h8000000000000000, 64'hx / 64'h8000000000000000, 64'hx % 64'h8000000000000000,
             64'hx ** 64'h8000000000000000, 64'shx / 64'h8000000000000000, 64'shx ** 64'h8000000000000000, 64'hx ** 64'sh8000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 6,
             -64'shx, 64'shx + 64'sh7fffffffffffffff, 64'shx - 64'sh7fffffffffffffff, 64'shx * 64'sh7fffffffffffffff, 64'shx / 64'sh7fffffffffffffff,
             64'shx % 64'sh7fffffffffffffff, 64'shx ** 64'sh7fffffffffffffff, 64'hx * 64'h7fffffffffffffff, 64'hx / 64'h7fffffffffffffff, 64'hx % 64'h7fffffffffffffff,
             64'hx ** 64'h7fffffffffffffff, 64'shx / 64'h7fffffffffffffff, 64'shx ** 64'h7fffffffffffffff, 64'hx ** 64'sh7fffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 10,
             -64'shx, 64'shx + 64'shfffffffffffffffd, 64'shx - 64'shfffffffffffffffd, 64'shx * 64'shfffffffffffffffd, 64'shx / 64'shfffffffffffffffd,
             64'shx % 64'shfffffffffffffffd, 64'shx ** 64'shfffffffffffffffd, 64'hx * 64'hfffffffffffffffd, 64'hx / 64'hfffffffffffffffd, 64'hx % 64'hfffffffffffffffd,
             64'hx ** 64'hfffffffffffffffd, 64'shx / 64'hfffffffffffffffd, 64'shx ** 64'hfffffffffffffffd, 64'hx ** 64'shfffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 64, 11, 11,
             -64'shx, 64'shx + 64'shx, 64'shx - 64'shx, 64'shx * 64'shx, 64'shx / 64'shx,
             64'shx % 64'shx, 64'shx ** 64'shx, 64'hx * 64'hx, 64'hx / 64'hx, 64'hx % 64'hx,
             64'hx ** 64'hx, 64'shx / 64'hx, 64'shx ** 64'hx, 64'hx ** 64'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 0,
             -65'sh0, 65'sh0 + 65'sh0, 65'sh0 - 65'sh0, 65'sh0 * 65'sh0, 65'sh0 / 65'sh0,
             65'sh0 % 65'sh0, 65'sh0 ** 65'sh0, 65'h0 * 65'h0, 65'h0 / 65'h0, 65'h0 % 65'h0,
             65'h0 ** 65'h0, 65'sh0 / 65'h0, 65'sh0 ** 65'h0, 65'h0 ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 0,
             65'h0 << 65'h0, 65'h0 >> 65'h0, 65'sh0 >>> 65'h0, 65'sh0 >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 1,
             -65'sh0, 65'sh0 + 65'sh1, 65'sh0 - 65'sh1, 65'sh0 * 65'sh1, 65'sh0 / 65'sh1,
             65'sh0 % 65'sh1, 65'sh0 ** 65'sh1, 65'h0 * 65'h1, 65'h0 / 65'h1, 65'h0 % 65'h1,
             65'h0 ** 65'h1, 65'sh0 / 65'h1, 65'sh0 ** 65'h1, 65'h0 ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 1,
             65'h0 << 65'h1, 65'h0 >> 65'h1, 65'sh0 >>> 65'h1, 65'sh0 >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 4,
             -65'sh0, 65'sh0 + 65'sh1ffffffffffffffff, 65'sh0 - 65'sh1ffffffffffffffff, 65'sh0 * 65'sh1ffffffffffffffff, 65'sh0 / 65'sh1ffffffffffffffff,
             65'sh0 % 65'sh1ffffffffffffffff, 65'sh0 ** 65'sh1ffffffffffffffff, 65'h0 * 65'h1ffffffffffffffff, 65'h0 / 65'h1ffffffffffffffff, 65'h0 % 65'h1ffffffffffffffff,
             65'h0 ** 65'h1ffffffffffffffff, 65'sh0 / 65'h1ffffffffffffffff, 65'sh0 ** 65'h1ffffffffffffffff, 65'h0 ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 4,
             65'h0 << 65'h1ffffffffffffffff, 65'h0 >> 65'h1ffffffffffffffff, 65'sh0 >>> 65'h1ffffffffffffffff, 65'sh0 >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 5,
             -65'sh0, 65'sh0 + 65'sh10000000000000000, 65'sh0 - 65'sh10000000000000000, 65'sh0 * 65'sh10000000000000000, 65'sh0 / 65'sh10000000000000000,
             65'sh0 % 65'sh10000000000000000, 65'sh0 ** 65'sh10000000000000000, 65'h0 * 65'h10000000000000000, 65'h0 / 65'h10000000000000000, 65'h0 % 65'h10000000000000000,
             65'h0 ** 65'h10000000000000000, 65'sh0 / 65'h10000000000000000, 65'sh0 ** 65'h10000000000000000, 65'h0 ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 5,
             65'h0 << 65'h10000000000000000, 65'h0 >> 65'h10000000000000000, 65'sh0 >>> 65'h10000000000000000, 65'sh0 >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 6,
             -65'sh0, 65'sh0 + 65'shffffffffffffffff, 65'sh0 - 65'shffffffffffffffff, 65'sh0 * 65'shffffffffffffffff, 65'sh0 / 65'shffffffffffffffff,
             65'sh0 % 65'shffffffffffffffff, 65'sh0 ** 65'shffffffffffffffff, 65'h0 * 65'hffffffffffffffff, 65'h0 / 65'hffffffffffffffff, 65'h0 % 65'hffffffffffffffff,
             65'h0 ** 65'hffffffffffffffff, 65'sh0 / 65'hffffffffffffffff, 65'sh0 ** 65'hffffffffffffffff, 65'h0 ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 6,
             65'h0 << 65'hffffffffffffffff, 65'h0 >> 65'hffffffffffffffff, 65'sh0 >>> 65'hffffffffffffffff, 65'sh0 >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 10,
             -65'sh0, 65'sh0 + 65'sh1fffffffffffffffd, 65'sh0 - 65'sh1fffffffffffffffd, 65'sh0 * 65'sh1fffffffffffffffd, 65'sh0 / 65'sh1fffffffffffffffd,
             65'sh0 % 65'sh1fffffffffffffffd, 65'sh0 ** 65'sh1fffffffffffffffd, 65'h0 * 65'h1fffffffffffffffd, 65'h0 / 65'h1fffffffffffffffd, 65'h0 % 65'h1fffffffffffffffd,
             65'h0 ** 65'h1fffffffffffffffd, 65'sh0 / 65'h1fffffffffffffffd, 65'sh0 ** 65'h1fffffffffffffffd, 65'h0 ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 10,
             65'h0 << 65'h1fffffffffffffffd, 65'h0 >> 65'h1fffffffffffffffd, 65'sh0 >>> 65'h1fffffffffffffffd, 65'sh0 >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 0, 11,
             -65'sh0, 65'sh0 + 65'shx, 65'sh0 - 65'shx, 65'sh0 * 65'shx, 65'sh0 / 65'shx,
             65'sh0 % 65'shx, 65'sh0 ** 65'shx, 65'h0 * 65'hx, 65'h0 / 65'hx, 65'h0 % 65'hx,
             65'h0 ** 65'hx, 65'sh0 / 65'hx, 65'sh0 ** 65'hx, 65'h0 ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 0, 11,
             65'h0 << 65'hx, 65'h0 >> 65'hx, 65'sh0 >>> 65'hx, 65'sh0 >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 0,
             -65'sh1, 65'sh1 + 65'sh0, 65'sh1 - 65'sh0, 65'sh1 * 65'sh0, 65'sh1 / 65'sh0,
             65'sh1 % 65'sh0, 65'sh1 ** 65'sh0, 65'h1 * 65'h0, 65'h1 / 65'h0, 65'h1 % 65'h0,
             65'h1 ** 65'h0, 65'sh1 / 65'h0, 65'sh1 ** 65'h0, 65'h1 ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 0,
             65'h1 << 65'h0, 65'h1 >> 65'h0, 65'sh1 >>> 65'h0, 65'sh1 >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 1,
             -65'sh1, 65'sh1 + 65'sh1, 65'sh1 - 65'sh1, 65'sh1 * 65'sh1, 65'sh1 / 65'sh1,
             65'sh1 % 65'sh1, 65'sh1 ** 65'sh1, 65'h1 * 65'h1, 65'h1 / 65'h1, 65'h1 % 65'h1,
             65'h1 ** 65'h1, 65'sh1 / 65'h1, 65'sh1 ** 65'h1, 65'h1 ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 1,
             65'h1 << 65'h1, 65'h1 >> 65'h1, 65'sh1 >>> 65'h1, 65'sh1 >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 4,
             -65'sh1, 65'sh1 + 65'sh1ffffffffffffffff, 65'sh1 - 65'sh1ffffffffffffffff, 65'sh1 * 65'sh1ffffffffffffffff, 65'sh1 / 65'sh1ffffffffffffffff,
             65'sh1 % 65'sh1ffffffffffffffff, 65'sh1 ** 65'sh1ffffffffffffffff, 65'h1 * 65'h1ffffffffffffffff, 65'h1 / 65'h1ffffffffffffffff, 65'h1 % 65'h1ffffffffffffffff,
             65'h1 ** 65'h1ffffffffffffffff, 65'sh1 / 65'h1ffffffffffffffff, 65'sh1 ** 65'h1ffffffffffffffff, 65'h1 ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 4,
             65'h1 << 65'h1ffffffffffffffff, 65'h1 >> 65'h1ffffffffffffffff, 65'sh1 >>> 65'h1ffffffffffffffff, 65'sh1 >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 5,
             -65'sh1, 65'sh1 + 65'sh10000000000000000, 65'sh1 - 65'sh10000000000000000, 65'sh1 * 65'sh10000000000000000, 65'sh1 / 65'sh10000000000000000,
             65'sh1 % 65'sh10000000000000000, 65'sh1 ** 65'sh10000000000000000, 65'h1 * 65'h10000000000000000, 65'h1 / 65'h10000000000000000, 65'h1 % 65'h10000000000000000,
             65'h1 ** 65'h10000000000000000, 65'sh1 / 65'h10000000000000000, 65'sh1 ** 65'h10000000000000000, 65'h1 ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 5,
             65'h1 << 65'h10000000000000000, 65'h1 >> 65'h10000000000000000, 65'sh1 >>> 65'h10000000000000000, 65'sh1 >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 6,
             -65'sh1, 65'sh1 + 65'shffffffffffffffff, 65'sh1 - 65'shffffffffffffffff, 65'sh1 * 65'shffffffffffffffff, 65'sh1 / 65'shffffffffffffffff,
             65'sh1 % 65'shffffffffffffffff, 65'sh1 ** 65'shffffffffffffffff, 65'h1 * 65'hffffffffffffffff, 65'h1 / 65'hffffffffffffffff, 65'h1 % 65'hffffffffffffffff,
             65'h1 ** 65'hffffffffffffffff, 65'sh1 / 65'hffffffffffffffff, 65'sh1 ** 65'hffffffffffffffff, 65'h1 ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 6,
             65'h1 << 65'hffffffffffffffff, 65'h1 >> 65'hffffffffffffffff, 65'sh1 >>> 65'hffffffffffffffff, 65'sh1 >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 10,
             -65'sh1, 65'sh1 + 65'sh1fffffffffffffffd, 65'sh1 - 65'sh1fffffffffffffffd, 65'sh1 * 65'sh1fffffffffffffffd, 65'sh1 / 65'sh1fffffffffffffffd,
             65'sh1 % 65'sh1fffffffffffffffd, 65'sh1 ** 65'sh1fffffffffffffffd, 65'h1 * 65'h1fffffffffffffffd, 65'h1 / 65'h1fffffffffffffffd, 65'h1 % 65'h1fffffffffffffffd,
             65'h1 ** 65'h1fffffffffffffffd, 65'sh1 / 65'h1fffffffffffffffd, 65'sh1 ** 65'h1fffffffffffffffd, 65'h1 ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 10,
             65'h1 << 65'h1fffffffffffffffd, 65'h1 >> 65'h1fffffffffffffffd, 65'sh1 >>> 65'h1fffffffffffffffd, 65'sh1 >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 1, 11,
             -65'sh1, 65'sh1 + 65'shx, 65'sh1 - 65'shx, 65'sh1 * 65'shx, 65'sh1 / 65'shx,
             65'sh1 % 65'shx, 65'sh1 ** 65'shx, 65'h1 * 65'hx, 65'h1 / 65'hx, 65'h1 % 65'hx,
             65'h1 ** 65'hx, 65'sh1 / 65'hx, 65'sh1 ** 65'hx, 65'h1 ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 1, 11,
             65'h1 << 65'hx, 65'h1 >> 65'hx, 65'sh1 >>> 65'hx, 65'sh1 >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 0,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'sh0, 65'sh1ffffffffffffffff - 65'sh0, 65'sh1ffffffffffffffff * 65'sh0, 65'sh1ffffffffffffffff / 65'sh0,
             65'sh1ffffffffffffffff % 65'sh0, 65'sh1ffffffffffffffff ** 65'sh0, 65'h1ffffffffffffffff * 65'h0, 65'h1ffffffffffffffff / 65'h0, 65'h1ffffffffffffffff % 65'h0,
             65'h1ffffffffffffffff ** 65'h0, 65'sh1ffffffffffffffff / 65'h0, 65'sh1ffffffffffffffff ** 65'h0, 65'h1ffffffffffffffff ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 0,
             65'h1ffffffffffffffff << 65'h0, 65'h1ffffffffffffffff >> 65'h0, 65'sh1ffffffffffffffff >>> 65'h0, 65'sh1ffffffffffffffff >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 1,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'sh1, 65'sh1ffffffffffffffff - 65'sh1, 65'sh1ffffffffffffffff * 65'sh1, 65'sh1ffffffffffffffff / 65'sh1,
             65'sh1ffffffffffffffff % 65'sh1, 65'sh1ffffffffffffffff ** 65'sh1, 65'h1ffffffffffffffff * 65'h1, 65'h1ffffffffffffffff / 65'h1, 65'h1ffffffffffffffff % 65'h1,
             65'h1ffffffffffffffff ** 65'h1, 65'sh1ffffffffffffffff / 65'h1, 65'sh1ffffffffffffffff ** 65'h1, 65'h1ffffffffffffffff ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 1,
             65'h1ffffffffffffffff << 65'h1, 65'h1ffffffffffffffff >> 65'h1, 65'sh1ffffffffffffffff >>> 65'h1, 65'sh1ffffffffffffffff >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 4,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff - 65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff * 65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff / 65'sh1ffffffffffffffff,
             65'sh1ffffffffffffffff % 65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff ** 65'sh1ffffffffffffffff, 65'h1ffffffffffffffff * 65'h1ffffffffffffffff, 65'h1ffffffffffffffff / 65'h1ffffffffffffffff, 65'h1ffffffffffffffff % 65'h1ffffffffffffffff,
             65'h1ffffffffffffffff ** 65'h1ffffffffffffffff, 65'sh1ffffffffffffffff / 65'h1ffffffffffffffff, 65'sh1ffffffffffffffff ** 65'h1ffffffffffffffff, 65'h1ffffffffffffffff ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 4,
             65'h1ffffffffffffffff << 65'h1ffffffffffffffff, 65'h1ffffffffffffffff >> 65'h1ffffffffffffffff, 65'sh1ffffffffffffffff >>> 65'h1ffffffffffffffff, 65'sh1ffffffffffffffff >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 5,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'sh10000000000000000, 65'sh1ffffffffffffffff - 65'sh10000000000000000, 65'sh1ffffffffffffffff * 65'sh10000000000000000, 65'sh1ffffffffffffffff / 65'sh10000000000000000,
             65'sh1ffffffffffffffff % 65'sh10000000000000000, 65'sh1ffffffffffffffff ** 65'sh10000000000000000, 65'h1ffffffffffffffff * 65'h10000000000000000, 65'h1ffffffffffffffff / 65'h10000000000000000, 65'h1ffffffffffffffff % 65'h10000000000000000,
             65'h1ffffffffffffffff ** 65'h10000000000000000, 65'sh1ffffffffffffffff / 65'h10000000000000000, 65'sh1ffffffffffffffff ** 65'h10000000000000000, 65'h1ffffffffffffffff ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 5,
             65'h1ffffffffffffffff << 65'h10000000000000000, 65'h1ffffffffffffffff >> 65'h10000000000000000, 65'sh1ffffffffffffffff >>> 65'h10000000000000000, 65'sh1ffffffffffffffff >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 6,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'shffffffffffffffff, 65'sh1ffffffffffffffff - 65'shffffffffffffffff, 65'sh1ffffffffffffffff * 65'shffffffffffffffff, 65'sh1ffffffffffffffff / 65'shffffffffffffffff,
             65'sh1ffffffffffffffff % 65'shffffffffffffffff, 65'sh1ffffffffffffffff ** 65'shffffffffffffffff, 65'h1ffffffffffffffff * 65'hffffffffffffffff, 65'h1ffffffffffffffff / 65'hffffffffffffffff, 65'h1ffffffffffffffff % 65'hffffffffffffffff,
             65'h1ffffffffffffffff ** 65'hffffffffffffffff, 65'sh1ffffffffffffffff / 65'hffffffffffffffff, 65'sh1ffffffffffffffff ** 65'hffffffffffffffff, 65'h1ffffffffffffffff ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 6,
             65'h1ffffffffffffffff << 65'hffffffffffffffff, 65'h1ffffffffffffffff >> 65'hffffffffffffffff, 65'sh1ffffffffffffffff >>> 65'hffffffffffffffff, 65'sh1ffffffffffffffff >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 10,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'sh1fffffffffffffffd, 65'sh1ffffffffffffffff - 65'sh1fffffffffffffffd, 65'sh1ffffffffffffffff * 65'sh1fffffffffffffffd, 65'sh1ffffffffffffffff / 65'sh1fffffffffffffffd,
             65'sh1ffffffffffffffff % 65'sh1fffffffffffffffd, 65'sh1ffffffffffffffff ** 65'sh1fffffffffffffffd, 65'h1ffffffffffffffff * 65'h1fffffffffffffffd, 65'h1ffffffffffffffff / 65'h1fffffffffffffffd, 65'h1ffffffffffffffff % 65'h1fffffffffffffffd,
             65'h1ffffffffffffffff ** 65'h1fffffffffffffffd, 65'sh1ffffffffffffffff / 65'h1fffffffffffffffd, 65'sh1ffffffffffffffff ** 65'h1fffffffffffffffd, 65'h1ffffffffffffffff ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 10,
             65'h1ffffffffffffffff << 65'h1fffffffffffffffd, 65'h1ffffffffffffffff >> 65'h1fffffffffffffffd, 65'sh1ffffffffffffffff >>> 65'h1fffffffffffffffd, 65'sh1ffffffffffffffff >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 4, 11,
             -65'sh1ffffffffffffffff, 65'sh1ffffffffffffffff + 65'shx, 65'sh1ffffffffffffffff - 65'shx, 65'sh1ffffffffffffffff * 65'shx, 65'sh1ffffffffffffffff / 65'shx,
             65'sh1ffffffffffffffff % 65'shx, 65'sh1ffffffffffffffff ** 65'shx, 65'h1ffffffffffffffff * 65'hx, 65'h1ffffffffffffffff / 65'hx, 65'h1ffffffffffffffff % 65'hx,
             65'h1ffffffffffffffff ** 65'hx, 65'sh1ffffffffffffffff / 65'hx, 65'sh1ffffffffffffffff ** 65'hx, 65'h1ffffffffffffffff ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 4, 11,
             65'h1ffffffffffffffff << 65'hx, 65'h1ffffffffffffffff >> 65'hx, 65'sh1ffffffffffffffff >>> 65'hx, 65'sh1ffffffffffffffff >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 0,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'sh0, 65'sh10000000000000000 - 65'sh0, 65'sh10000000000000000 * 65'sh0, 65'sh10000000000000000 / 65'sh0,
             65'sh10000000000000000 % 65'sh0, 65'sh10000000000000000 ** 65'sh0, 65'h10000000000000000 * 65'h0, 65'h10000000000000000 / 65'h0, 65'h10000000000000000 % 65'h0,
             65'h10000000000000000 ** 65'h0, 65'sh10000000000000000 / 65'h0, 65'sh10000000000000000 ** 65'h0, 65'h10000000000000000 ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 0,
             65'h10000000000000000 << 65'h0, 65'h10000000000000000 >> 65'h0, 65'sh10000000000000000 >>> 65'h0, 65'sh10000000000000000 >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 1,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'sh1, 65'sh10000000000000000 - 65'sh1, 65'sh10000000000000000 * 65'sh1, 65'sh10000000000000000 / 65'sh1,
             65'sh10000000000000000 % 65'sh1, 65'sh10000000000000000 ** 65'sh1, 65'h10000000000000000 * 65'h1, 65'h10000000000000000 / 65'h1, 65'h10000000000000000 % 65'h1,
             65'h10000000000000000 ** 65'h1, 65'sh10000000000000000 / 65'h1, 65'sh10000000000000000 ** 65'h1, 65'h10000000000000000 ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 1,
             65'h10000000000000000 << 65'h1, 65'h10000000000000000 >> 65'h1, 65'sh10000000000000000 >>> 65'h1, 65'sh10000000000000000 >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 4,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'sh1ffffffffffffffff, 65'sh10000000000000000 - 65'sh1ffffffffffffffff, 65'sh10000000000000000 * 65'sh1ffffffffffffffff, 65'sh10000000000000000 / 65'sh1ffffffffffffffff,
             65'sh10000000000000000 % 65'sh1ffffffffffffffff, 65'sh10000000000000000 ** 65'sh1ffffffffffffffff, 65'h10000000000000000 * 65'h1ffffffffffffffff, 65'h10000000000000000 / 65'h1ffffffffffffffff, 65'h10000000000000000 % 65'h1ffffffffffffffff,
             65'h10000000000000000 ** 65'h1ffffffffffffffff, 65'sh10000000000000000 / 65'h1ffffffffffffffff, 65'sh10000000000000000 ** 65'h1ffffffffffffffff, 65'h10000000000000000 ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 4,
             65'h10000000000000000 << 65'h1ffffffffffffffff, 65'h10000000000000000 >> 65'h1ffffffffffffffff, 65'sh10000000000000000 >>> 65'h1ffffffffffffffff, 65'sh10000000000000000 >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 5,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'sh10000000000000000, 65'sh10000000000000000 - 65'sh10000000000000000, 65'sh10000000000000000 * 65'sh10000000000000000, 65'sh10000000000000000 / 65'sh10000000000000000,
             65'sh10000000000000000 % 65'sh10000000000000000, 65'sh10000000000000000 ** 65'sh10000000000000000, 65'h10000000000000000 * 65'h10000000000000000, 65'h10000000000000000 / 65'h10000000000000000, 65'h10000000000000000 % 65'h10000000000000000,
             65'h10000000000000000 ** 65'h10000000000000000, 65'sh10000000000000000 / 65'h10000000000000000, 65'sh10000000000000000 ** 65'h10000000000000000, 65'h10000000000000000 ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 5,
             65'h10000000000000000 << 65'h10000000000000000, 65'h10000000000000000 >> 65'h10000000000000000, 65'sh10000000000000000 >>> 65'h10000000000000000, 65'sh10000000000000000 >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 6,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'shffffffffffffffff, 65'sh10000000000000000 - 65'shffffffffffffffff, 65'sh10000000000000000 * 65'shffffffffffffffff, 65'sh10000000000000000 / 65'shffffffffffffffff,
             65'sh10000000000000000 % 65'shffffffffffffffff, 65'sh10000000000000000 ** 65'shffffffffffffffff, 65'h10000000000000000 * 65'hffffffffffffffff, 65'h10000000000000000 / 65'hffffffffffffffff, 65'h10000000000000000 % 65'hffffffffffffffff,
             65'h10000000000000000 ** 65'hffffffffffffffff, 65'sh10000000000000000 / 65'hffffffffffffffff, 65'sh10000000000000000 ** 65'hffffffffffffffff, 65'h10000000000000000 ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 6,
             65'h10000000000000000 << 65'hffffffffffffffff, 65'h10000000000000000 >> 65'hffffffffffffffff, 65'sh10000000000000000 >>> 65'hffffffffffffffff, 65'sh10000000000000000 >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 10,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'sh1fffffffffffffffd, 65'sh10000000000000000 - 65'sh1fffffffffffffffd, 65'sh10000000000000000 * 65'sh1fffffffffffffffd, 65'sh10000000000000000 / 65'sh1fffffffffffffffd,
             65'sh10000000000000000 % 65'sh1fffffffffffffffd, 65'sh10000000000000000 ** 65'sh1fffffffffffffffd, 65'h10000000000000000 * 65'h1fffffffffffffffd, 65'h10000000000000000 / 65'h1fffffffffffffffd, 65'h10000000000000000 % 65'h1fffffffffffffffd,
             65'h10000000000000000 ** 65'h1fffffffffffffffd, 65'sh10000000000000000 / 65'h1fffffffffffffffd, 65'sh10000000000000000 ** 65'h1fffffffffffffffd, 65'h10000000000000000 ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 10,
             65'h10000000000000000 << 65'h1fffffffffffffffd, 65'h10000000000000000 >> 65'h1fffffffffffffffd, 65'sh10000000000000000 >>> 65'h1fffffffffffffffd, 65'sh10000000000000000 >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 5, 11,
             -65'sh10000000000000000, 65'sh10000000000000000 + 65'shx, 65'sh10000000000000000 - 65'shx, 65'sh10000000000000000 * 65'shx, 65'sh10000000000000000 / 65'shx,
             65'sh10000000000000000 % 65'shx, 65'sh10000000000000000 ** 65'shx, 65'h10000000000000000 * 65'hx, 65'h10000000000000000 / 65'hx, 65'h10000000000000000 % 65'hx,
             65'h10000000000000000 ** 65'hx, 65'sh10000000000000000 / 65'hx, 65'sh10000000000000000 ** 65'hx, 65'h10000000000000000 ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 5, 11,
             65'h10000000000000000 << 65'hx, 65'h10000000000000000 >> 65'hx, 65'sh10000000000000000 >>> 65'hx, 65'sh10000000000000000 >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 0,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'sh0, 65'shffffffffffffffff - 65'sh0, 65'shffffffffffffffff * 65'sh0, 65'shffffffffffffffff / 65'sh0,
             65'shffffffffffffffff % 65'sh0, 65'shffffffffffffffff ** 65'sh0, 65'hffffffffffffffff * 65'h0, 65'hffffffffffffffff / 65'h0, 65'hffffffffffffffff % 65'h0,
             65'hffffffffffffffff ** 65'h0, 65'shffffffffffffffff / 65'h0, 65'shffffffffffffffff ** 65'h0, 65'hffffffffffffffff ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 0,
             65'hffffffffffffffff << 65'h0, 65'hffffffffffffffff >> 65'h0, 65'shffffffffffffffff >>> 65'h0, 65'shffffffffffffffff >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 1,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'sh1, 65'shffffffffffffffff - 65'sh1, 65'shffffffffffffffff * 65'sh1, 65'shffffffffffffffff / 65'sh1,
             65'shffffffffffffffff % 65'sh1, 65'shffffffffffffffff ** 65'sh1, 65'hffffffffffffffff * 65'h1, 65'hffffffffffffffff / 65'h1, 65'hffffffffffffffff % 65'h1,
             65'hffffffffffffffff ** 65'h1, 65'shffffffffffffffff / 65'h1, 65'shffffffffffffffff ** 65'h1, 65'hffffffffffffffff ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 1,
             65'hffffffffffffffff << 65'h1, 65'hffffffffffffffff >> 65'h1, 65'shffffffffffffffff >>> 65'h1, 65'shffffffffffffffff >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 4,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'sh1ffffffffffffffff, 65'shffffffffffffffff - 65'sh1ffffffffffffffff, 65'shffffffffffffffff * 65'sh1ffffffffffffffff, 65'shffffffffffffffff / 65'sh1ffffffffffffffff,
             65'shffffffffffffffff % 65'sh1ffffffffffffffff, 65'shffffffffffffffff ** 65'sh1ffffffffffffffff, 65'hffffffffffffffff * 65'h1ffffffffffffffff, 65'hffffffffffffffff / 65'h1ffffffffffffffff, 65'hffffffffffffffff % 65'h1ffffffffffffffff,
             65'hffffffffffffffff ** 65'h1ffffffffffffffff, 65'shffffffffffffffff / 65'h1ffffffffffffffff, 65'shffffffffffffffff ** 65'h1ffffffffffffffff, 65'hffffffffffffffff ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 4,
             65'hffffffffffffffff << 65'h1ffffffffffffffff, 65'hffffffffffffffff >> 65'h1ffffffffffffffff, 65'shffffffffffffffff >>> 65'h1ffffffffffffffff, 65'shffffffffffffffff >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 5,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'sh10000000000000000, 65'shffffffffffffffff - 65'sh10000000000000000, 65'shffffffffffffffff * 65'sh10000000000000000, 65'shffffffffffffffff / 65'sh10000000000000000,
             65'shffffffffffffffff % 65'sh10000000000000000, 65'shffffffffffffffff ** 65'sh10000000000000000, 65'hffffffffffffffff * 65'h10000000000000000, 65'hffffffffffffffff / 65'h10000000000000000, 65'hffffffffffffffff % 65'h10000000000000000,
             65'hffffffffffffffff ** 65'h10000000000000000, 65'shffffffffffffffff / 65'h10000000000000000, 65'shffffffffffffffff ** 65'h10000000000000000, 65'hffffffffffffffff ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 5,
             65'hffffffffffffffff << 65'h10000000000000000, 65'hffffffffffffffff >> 65'h10000000000000000, 65'shffffffffffffffff >>> 65'h10000000000000000, 65'shffffffffffffffff >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 6,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'shffffffffffffffff, 65'shffffffffffffffff - 65'shffffffffffffffff, 65'shffffffffffffffff * 65'shffffffffffffffff, 65'shffffffffffffffff / 65'shffffffffffffffff,
             65'shffffffffffffffff % 65'shffffffffffffffff, 65'shffffffffffffffff ** 65'shffffffffffffffff, 65'hffffffffffffffff * 65'hffffffffffffffff, 65'hffffffffffffffff / 65'hffffffffffffffff, 65'hffffffffffffffff % 65'hffffffffffffffff,
             65'hffffffffffffffff ** 65'hffffffffffffffff, 65'shffffffffffffffff / 65'hffffffffffffffff, 65'shffffffffffffffff ** 65'hffffffffffffffff, 65'hffffffffffffffff ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 6,
             65'hffffffffffffffff << 65'hffffffffffffffff, 65'hffffffffffffffff >> 65'hffffffffffffffff, 65'shffffffffffffffff >>> 65'hffffffffffffffff, 65'shffffffffffffffff >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 10,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'sh1fffffffffffffffd, 65'shffffffffffffffff - 65'sh1fffffffffffffffd, 65'shffffffffffffffff * 65'sh1fffffffffffffffd, 65'shffffffffffffffff / 65'sh1fffffffffffffffd,
             65'shffffffffffffffff % 65'sh1fffffffffffffffd, 65'shffffffffffffffff ** 65'sh1fffffffffffffffd, 65'hffffffffffffffff * 65'h1fffffffffffffffd, 65'hffffffffffffffff / 65'h1fffffffffffffffd, 65'hffffffffffffffff % 65'h1fffffffffffffffd,
             65'hffffffffffffffff ** 65'h1fffffffffffffffd, 65'shffffffffffffffff / 65'h1fffffffffffffffd, 65'shffffffffffffffff ** 65'h1fffffffffffffffd, 65'hffffffffffffffff ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 10,
             65'hffffffffffffffff << 65'h1fffffffffffffffd, 65'hffffffffffffffff >> 65'h1fffffffffffffffd, 65'shffffffffffffffff >>> 65'h1fffffffffffffffd, 65'shffffffffffffffff >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 6, 11,
             -65'shffffffffffffffff, 65'shffffffffffffffff + 65'shx, 65'shffffffffffffffff - 65'shx, 65'shffffffffffffffff * 65'shx, 65'shffffffffffffffff / 65'shx,
             65'shffffffffffffffff % 65'shx, 65'shffffffffffffffff ** 65'shx, 65'hffffffffffffffff * 65'hx, 65'hffffffffffffffff / 65'hx, 65'hffffffffffffffff % 65'hx,
             65'hffffffffffffffff ** 65'hx, 65'shffffffffffffffff / 65'hx, 65'shffffffffffffffff ** 65'hx, 65'hffffffffffffffff ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 6, 11,
             65'hffffffffffffffff << 65'hx, 65'hffffffffffffffff >> 65'hx, 65'shffffffffffffffff >>> 65'hx, 65'shffffffffffffffff >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 0,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'sh0, 65'sh1fffffffffffffffd - 65'sh0, 65'sh1fffffffffffffffd * 65'sh0, 65'sh1fffffffffffffffd / 65'sh0,
             65'sh1fffffffffffffffd % 65'sh0, 65'sh1fffffffffffffffd ** 65'sh0, 65'h1fffffffffffffffd * 65'h0, 65'h1fffffffffffffffd / 65'h0, 65'h1fffffffffffffffd % 65'h0,
             65'h1fffffffffffffffd ** 65'h0, 65'sh1fffffffffffffffd / 65'h0, 65'sh1fffffffffffffffd ** 65'h0, 65'h1fffffffffffffffd ** 65'sh0);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 0,
             65'h1fffffffffffffffd << 65'h0, 65'h1fffffffffffffffd >> 65'h0, 65'sh1fffffffffffffffd >>> 65'h0, 65'sh1fffffffffffffffd >>> 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 1,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'sh1, 65'sh1fffffffffffffffd - 65'sh1, 65'sh1fffffffffffffffd * 65'sh1, 65'sh1fffffffffffffffd / 65'sh1,
             65'sh1fffffffffffffffd % 65'sh1, 65'sh1fffffffffffffffd ** 65'sh1, 65'h1fffffffffffffffd * 65'h1, 65'h1fffffffffffffffd / 65'h1, 65'h1fffffffffffffffd % 65'h1,
             65'h1fffffffffffffffd ** 65'h1, 65'sh1fffffffffffffffd / 65'h1, 65'sh1fffffffffffffffd ** 65'h1, 65'h1fffffffffffffffd ** 65'sh1);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 1,
             65'h1fffffffffffffffd << 65'h1, 65'h1fffffffffffffffd >> 65'h1, 65'sh1fffffffffffffffd >>> 65'h1, 65'sh1fffffffffffffffd >>> 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 4,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'sh1ffffffffffffffff, 65'sh1fffffffffffffffd - 65'sh1ffffffffffffffff, 65'sh1fffffffffffffffd * 65'sh1ffffffffffffffff, 65'sh1fffffffffffffffd / 65'sh1ffffffffffffffff,
             65'sh1fffffffffffffffd % 65'sh1ffffffffffffffff, 65'sh1fffffffffffffffd ** 65'sh1ffffffffffffffff, 65'h1fffffffffffffffd * 65'h1ffffffffffffffff, 65'h1fffffffffffffffd / 65'h1ffffffffffffffff, 65'h1fffffffffffffffd % 65'h1ffffffffffffffff,
             65'h1fffffffffffffffd ** 65'h1ffffffffffffffff, 65'sh1fffffffffffffffd / 65'h1ffffffffffffffff, 65'sh1fffffffffffffffd ** 65'h1ffffffffffffffff, 65'h1fffffffffffffffd ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 4,
             65'h1fffffffffffffffd << 65'h1ffffffffffffffff, 65'h1fffffffffffffffd >> 65'h1ffffffffffffffff, 65'sh1fffffffffffffffd >>> 65'h1ffffffffffffffff, 65'sh1fffffffffffffffd >>> 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 5,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'sh10000000000000000, 65'sh1fffffffffffffffd - 65'sh10000000000000000, 65'sh1fffffffffffffffd * 65'sh10000000000000000, 65'sh1fffffffffffffffd / 65'sh10000000000000000,
             65'sh1fffffffffffffffd % 65'sh10000000000000000, 65'sh1fffffffffffffffd ** 65'sh10000000000000000, 65'h1fffffffffffffffd * 65'h10000000000000000, 65'h1fffffffffffffffd / 65'h10000000000000000, 65'h1fffffffffffffffd % 65'h10000000000000000,
             65'h1fffffffffffffffd ** 65'h10000000000000000, 65'sh1fffffffffffffffd / 65'h10000000000000000, 65'sh1fffffffffffffffd ** 65'h10000000000000000, 65'h1fffffffffffffffd ** 65'sh10000000000000000);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 5,
             65'h1fffffffffffffffd << 65'h10000000000000000, 65'h1fffffffffffffffd >> 65'h10000000000000000, 65'sh1fffffffffffffffd >>> 65'h10000000000000000, 65'sh1fffffffffffffffd >>> 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 6,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'shffffffffffffffff, 65'sh1fffffffffffffffd - 65'shffffffffffffffff, 65'sh1fffffffffffffffd * 65'shffffffffffffffff, 65'sh1fffffffffffffffd / 65'shffffffffffffffff,
             65'sh1fffffffffffffffd % 65'shffffffffffffffff, 65'sh1fffffffffffffffd ** 65'shffffffffffffffff, 65'h1fffffffffffffffd * 65'hffffffffffffffff, 65'h1fffffffffffffffd / 65'hffffffffffffffff, 65'h1fffffffffffffffd % 65'hffffffffffffffff,
             65'h1fffffffffffffffd ** 65'hffffffffffffffff, 65'sh1fffffffffffffffd / 65'hffffffffffffffff, 65'sh1fffffffffffffffd ** 65'hffffffffffffffff, 65'h1fffffffffffffffd ** 65'shffffffffffffffff);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 6,
             65'h1fffffffffffffffd << 65'hffffffffffffffff, 65'h1fffffffffffffffd >> 65'hffffffffffffffff, 65'sh1fffffffffffffffd >>> 65'hffffffffffffffff, 65'sh1fffffffffffffffd >>> 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 10,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd - 65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd * 65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd / 65'sh1fffffffffffffffd,
             65'sh1fffffffffffffffd % 65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd ** 65'sh1fffffffffffffffd, 65'h1fffffffffffffffd * 65'h1fffffffffffffffd, 65'h1fffffffffffffffd / 65'h1fffffffffffffffd, 65'h1fffffffffffffffd % 65'h1fffffffffffffffd,
             65'h1fffffffffffffffd ** 65'h1fffffffffffffffd, 65'sh1fffffffffffffffd / 65'h1fffffffffffffffd, 65'sh1fffffffffffffffd ** 65'h1fffffffffffffffd, 65'h1fffffffffffffffd ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 10,
             65'h1fffffffffffffffd << 65'h1fffffffffffffffd, 65'h1fffffffffffffffd >> 65'h1fffffffffffffffd, 65'sh1fffffffffffffffd >>> 65'h1fffffffffffffffd, 65'sh1fffffffffffffffd >>> 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 10, 11,
             -65'sh1fffffffffffffffd, 65'sh1fffffffffffffffd + 65'shx, 65'sh1fffffffffffffffd - 65'shx, 65'sh1fffffffffffffffd * 65'shx, 65'sh1fffffffffffffffd / 65'shx,
             65'sh1fffffffffffffffd % 65'shx, 65'sh1fffffffffffffffd ** 65'shx, 65'h1fffffffffffffffd * 65'hx, 65'h1fffffffffffffffd / 65'hx, 65'h1fffffffffffffffd % 65'hx,
             65'h1fffffffffffffffd ** 65'hx, 65'sh1fffffffffffffffd / 65'hx, 65'sh1fffffffffffffffd ** 65'hx, 65'h1fffffffffffffffd ** 65'shx);
    $display("%0d %0d %0d ks %h %h %h %h", 65, 10, 11,
             65'h1fffffffffffffffd << 65'hx, 65'h1fffffffffffffffd >> 65'hx, 65'sh1fffffffffffffffd >>> 65'hx, 65'sh1fffffffffffffffd >>> 65'shx);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 0,
             -65'shx, 65'shx + 65'sh0, 65'shx - 65'sh0, 65'shx * 65'sh0, 65'shx / 65'sh0,
             65'shx % 65'sh0, 65'shx ** 65'sh0, 65'hx * 65'h0, 65'hx / 65'h0, 65'hx % 65'h0,
             65'hx ** 65'h0, 65'shx / 65'h0, 65'shx ** 65'h0, 65'hx ** 65'sh0);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 1,
             -65'shx, 65'shx + 65'sh1, 65'shx - 65'sh1, 65'shx * 65'sh1, 65'shx / 65'sh1,
             65'shx % 65'sh1, 65'shx ** 65'sh1, 65'hx * 65'h1, 65'hx / 65'h1, 65'hx % 65'h1,
             65'hx ** 65'h1, 65'shx / 65'h1, 65'shx ** 65'h1, 65'hx ** 65'sh1);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 4,
             -65'shx, 65'shx + 65'sh1ffffffffffffffff, 65'shx - 65'sh1ffffffffffffffff, 65'shx * 65'sh1ffffffffffffffff, 65'shx / 65'sh1ffffffffffffffff,
             65'shx % 65'sh1ffffffffffffffff, 65'shx ** 65'sh1ffffffffffffffff, 65'hx * 65'h1ffffffffffffffff, 65'hx / 65'h1ffffffffffffffff, 65'hx % 65'h1ffffffffffffffff,
             65'hx ** 65'h1ffffffffffffffff, 65'shx / 65'h1ffffffffffffffff, 65'shx ** 65'h1ffffffffffffffff, 65'hx ** 65'sh1ffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 5,
             -65'shx, 65'shx + 65'sh10000000000000000, 65'shx - 65'sh10000000000000000, 65'shx * 65'sh10000000000000000, 65'shx / 65'sh10000000000000000,
             65'shx % 65'sh10000000000000000, 65'shx ** 65'sh10000000000000000, 65'hx * 65'h10000000000000000, 65'hx / 65'h10000000000000000, 65'hx % 65'h10000000000000000,
             65'hx ** 65'h10000000000000000, 65'shx / 65'h10000000000000000, 65'shx ** 65'h10000000000000000, 65'hx ** 65'sh10000000000000000);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 6,
             -65'shx, 65'shx + 65'shffffffffffffffff, 65'shx - 65'shffffffffffffffff, 65'shx * 65'shffffffffffffffff, 65'shx / 65'shffffffffffffffff,
             65'shx % 65'shffffffffffffffff, 65'shx ** 65'shffffffffffffffff, 65'hx * 65'hffffffffffffffff, 65'hx / 65'hffffffffffffffff, 65'hx % 65'hffffffffffffffff,
             65'hx ** 65'hffffffffffffffff, 65'shx / 65'hffffffffffffffff, 65'shx ** 65'hffffffffffffffff, 65'hx ** 65'shffffffffffffffff);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 10,
             -65'shx, 65'shx + 65'sh1fffffffffffffffd, 65'shx - 65'sh1fffffffffffffffd, 65'shx * 65'sh1fffffffffffffffd, 65'shx / 65'sh1fffffffffffffffd,
             65'shx % 65'sh1fffffffffffffffd, 65'shx ** 65'sh1fffffffffffffffd, 65'hx * 65'h1fffffffffffffffd, 65'hx / 65'h1fffffffffffffffd, 65'hx % 65'h1fffffffffffffffd,
             65'hx ** 65'h1fffffffffffffffd, 65'shx / 65'h1fffffffffffffffd, 65'shx ** 65'h1fffffffffffffffd, 65'hx ** 65'sh1fffffffffffffffd);
    $display("%0d %0d %0d k %h %h %h %h %h %h %h %h %h %h %h %h %h %h", 65, 11, 11,
             -65'shx, 65'shx + 65'shx, 65'shx - 65'shx, 65'shx * 65'shx, 65'shx / 65'shx,
             65'shx % 65'shx, 65'shx ** 65'shx, 65'hx * 65'hx, 65'hx / 65'hx, 65'hx % 65'hx,
             65'hx ** 65'hx, 65'shx / 65'hx, 65'shx ** 65'hx, 65'hx ** 65'shx);
    $finish(0);
  end
endmodule
