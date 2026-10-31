// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4: literal operands, so
// the default optimizer folds what it can while --no-opt evaluates at run time.
// Operand values follow arith_matrix.sv (indices 0, 1, 4, 5, 6, 10, 11).
// Width 64 of the four arith_constants_w*.sv fixtures: one width each keeps
// every process body small enough for C compilers to optimize quickly.
module tb;
  initial begin
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
    $finish(0);
  end
endmodule
