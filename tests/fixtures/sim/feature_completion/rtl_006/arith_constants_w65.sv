// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4: literal operands, so
// the default optimizer folds what it can while --no-opt evaluates at run time.
// Operand values follow arith_matrix.sv (indices 0, 1, 4, 5, 6, 10, 11).
// Width 65 of the four arith_constants_w*.sv fixtures: one width each keeps
// every process body small enough for C compilers to optimize quickly.
module tb;
  initial begin
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
