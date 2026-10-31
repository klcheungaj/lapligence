// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4: literal operands, so
// the default optimizer folds what it can while --no-opt evaluates at run time.
// Operand values follow arith_matrix.sv (indices 0, 1, 4, 5, 6, 10, 11).
// Width 32 of the four arith_constants_w*.sv fixtures: one width each keeps
// every process body small enough for C compilers to optimize quickly.
module tb;
  initial begin
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
    $finish(0);
  end
endmodule
