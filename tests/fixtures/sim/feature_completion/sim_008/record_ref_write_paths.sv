// Every store through a `ref` record formal bound to an array element reaches
// the element's readers at that store (SV 13.5.2, 9.4.3; decision S13-D4). Each
// path has its own array, so no other store can wake its waiter. See the readme.
`timescale 1ns/1ns
module tb;
  class C; int k; endclass
  typedef struct { int p; string t; } sub_t;
  typedef struct { string s; int n; int a[3]; int m; real r; C h; bit [7:0] b; sub_t u; } rec_t;
  class K; virtual task m(ref rec_t x); #1 x.n = 81; endtask endclass
  event ev; initial #3 ->ev;
  K k = new;
  C kc = new;
  task automatic out1(output int o, input int v); #1 o = v; endtask
  task automatic io1(inout int o); #1 o = o + 1; endtask
  task automatic outs(output string o); #1 o = "outs"; endtask
  task automatic out_rec(output rec_t o); #1 o.n = 82; endtask
  task automatic inner(ref int y); y = 1; #2 y = 2; endtask
  function automatic int fout(output int o, input int v); o = v; return v; endfunction
  function automatic rec_t mk(input int v); mk.n = v; endfunction
  function automatic void f13(ref rec_t x); x.n = 61; endfunction
  function automatic int f14(ref rec_t x); return x.n++; endfunction
  rec_t a0[2];
  task automatic t0(ref rec_t x); #1; x.n = #1 11; endtask
  rec_t a1[2];
  task automatic t1(ref rec_t x); #1; for (x.n = 21; x.n < 23; x.n++) begin #2; end endtask
  rec_t a2[2];
  task automatic t2(ref rec_t x); #1; out1(x.n, 31); #2; endtask
  rec_t a3[2];
  task automatic t3(ref rec_t x); #1; io1(x.n); #2; endtask
  rec_t a4[2];
  task automatic t4(ref rec_t x); #1; x.n = repeat (1) @(ev) 12; endtask
  rec_t a5[2];
  task automatic t5(ref rec_t x); #1; if (fout(x.n, 41) != 41) $display("no"); else #2; endtask
  rec_t a6[2];
  task automatic t6(ref rec_t x); #1; x.n = 2; #1; while (x.n-- > 0) #2; endtask
  rec_t a7[2];
  task automatic t7(ref rec_t x); int y; #1; y = x.n++; #1 x.n += 2; #1 $display("y %0d", y); endtask
  rec_t a8[2];
  task automatic t8(ref rec_t x); #1; {x.n, x.b} = 40'h0000003301; endtask
  rec_t a9[2];
  task automatic t9(ref rec_t x); #1; outs(x.s); endtask
  rec_t a10[2];
  task automatic t10(ref rec_t x); #1; $sformat(x.s, "%0d", 7); #1 $swrite(x.s, "%0d", 8); endtask
  rec_t a11[2];
  task automatic t11(ref rec_t x); #1; x.s = "abc"; #1 x.s.putc(0, "z"); #1 x.s[1] = "y"; #1 x.s.itoa(5); endtask
  rec_t a12[2];
  task automatic t12(ref rec_t x); rec_t y; y.n = 9; #1; x = y; endtask
  rec_t a13[2];
  task automatic t13(ref rec_t x); #1; x = mk(10); endtask
  rec_t a14[2];
  task automatic t14(ref rec_t x); #1; x.r = 1.5; endtask
  rec_t a15[2];
  task automatic t15(ref rec_t x); #1; x.h = new; endtask
  rec_t a16[2];
  task automatic t16(ref rec_t x); #1; x.b[2] = 1; #1 x.b[7:6] = 2'b11; endtask
  rec_t a17[2];
  task automatic t17(ref rec_t x); #1; x.u.p = 3; #1 x.u.t = "ut"; endtask
  rec_t a18[2];
  task automatic t18(ref rec_t x); int i = 2; #1; x.a[i] = 8; #1; x.a = '{1,2,3}; endtask
  rec_t a19[2];
  task automatic t19(ref rec_t x); #1; foreach (x.a[i]) x.a[i] = i + 5; endtask
  rec_t a20[2];
  task automatic t20(ref rec_t x); #1; if (!$cast(x.h, kc)) $display("no"); endtask
  rec_t a21[2];
  task automatic t21(ref rec_t x); #1; if ($sscanf("88", "%d", x.n) != 1) $display("no"); endtask
  rec_t a22[2];
  task automatic t22(ref rec_t x); mailbox #(int) mb = new; #1; mb.put(99); mb.get(x.n); endtask
  rec_t a23[2];
  task automatic t23(ref rec_t x); mailbox #(rec_t) mb = new; rec_t y; y.n = 98; #1; mb.put(y); mb.get(x); endtask
  rec_t a24[2];
  task automatic t24(ref rec_t x); #1; inner(x.m); endtask
  rec_t a25[2];
  task automatic t25(ref rec_t x); #1; f13(x); #2; endtask
  rec_t a26[2];
  task automatic t26(ref rec_t x); int y; #1; y = f14(x); #2 $display("y %0d", y); endtask
  rec_t a27[2];
  task automatic t27(ref rec_t x); #1; {>>{x.n}} = 32'd71; endtask
  rec_t a28[2];
  task automatic t28(ref rec_t x); #1; x.b++; #1 --x.b; #1 x.n = x.n; endtask
  rec_t a29[2];
  task automatic t29(ref rec_t x); #1; k.m(x); endtask
  rec_t a30[2];
  task automatic t30(ref rec_t x); #1; out_rec(x); #2; endtask
  // Reads through a formal publish nothing: this block runs once, at time 0.
  rec_t quiet[2];
  int fresh;
  always_comb begin
    fresh = quiet[1].n;
    $display("%0t quiet reader", $time);
  end
  task automatic peek_ref(const ref rec_t x, ref int y); y = x.n; endtask
  task automatic pass(input rec_t y); endtask
  task automatic reads(ref rec_t x);
    int y;
    string t;
    #1 y = x.n + x.a[1];
    #1 t = x.s;
    #1 y = x.s.len();
    #1 peek_ref(x, y);
    #1 pass(x);
    #1 if (x.n == 0 && x.s == "") y = 1;
    #1 $display("reads %0d", y);
  endtask
  // String members as method receivers and output actuals, without a formal.
  rec_t direct[2];
  task automatic members();
    rec_t l;
    l.s = "ab";
    l.s.putc(1, "c");
    $display("l %s", l.s);
    outs(l.s);
    $display("l %s", l.s);
    direct[1].s = "ab";
    direct[1].s.putc(0, "x");
    $display("e %s", direct[1].s);
    outs(direct[1].s);
    $sformat(direct[0].s, "%0d", 3);
    $display("e %s %s", direct[1].s, direct[0].s);
  endtask
  initial begin
    fork
      members();
      reads(quiet[1]);
      t0(a0[1]);
      begin wait (a0[1].n == 11) $display("%0t a0.0", $time); end
      t1(a1[1]);
      begin wait (a1[1].n == 21) $display("%0t a1.0", $time); wait (a1[1].n == 22) $display("%0t a1.1", $time); end
      t2(a2[1]);
      begin wait (a2[1].n == 31) $display("%0t a2.0", $time); end
      t3(a3[1]);
      begin wait (a3[1].n == 1) $display("%0t a3.0", $time); end
      t4(a4[1]);
      begin wait (a4[1].n == 12) $display("%0t a4.0", $time); end
      t5(a5[1]);
      begin wait (a5[1].n == 41) $display("%0t a5.0", $time); end
      t6(a6[1]);
      begin wait (a6[1].n == 1) $display("%0t a6.0", $time); wait (a6[1].n == 0) $display("%0t a6.1", $time); wait (a6[1].n == -1) $display("%0t a6.2", $time); end
      t7(a7[1]);
      begin wait (a7[1].n == 1) $display("%0t a7.0", $time); wait (a7[1].n == 3) $display("%0t a7.1", $time); end
      t8(a8[1]);
      begin wait (a8[1].n == 'h33 && a8[1].b == 1) $display("%0t a8.0", $time); end
      t9(a9[1]);
      begin wait (a9[1].s == "outs") $display("%0t a9.0", $time); end
      t10(a10[1]);
      begin wait (a10[1].s == "7") $display("%0t a10.0", $time); wait (a10[1].s == "8") $display("%0t a10.1", $time); end
      t11(a11[1]);
      begin wait (a11[1].s == "abc") $display("%0t a11.0", $time); wait (a11[1].s == "zbc") $display("%0t a11.1", $time); wait (a11[1].s == "zyc") $display("%0t a11.2", $time); wait (a11[1].s == "5") $display("%0t a11.3", $time); end
      t12(a12[1]);
      begin wait (a12[1].n == 9) $display("%0t a12.0", $time); end
      t13(a13[1]);
      begin wait (a13[1].n == 10) $display("%0t a13.0", $time); end
      t14(a14[1]);
      begin wait (a14[1].r == 1.5) $display("%0t a14.0", $time); end
      t15(a15[1]);
      begin wait (a15[1].h != null) $display("%0t a15.0", $time); end
      t16(a16[1]);
      begin wait (a16[1].b[2]) $display("%0t a16.0", $time); wait (a16[1].b[7]) $display("%0t a16.1", $time); end
      t17(a17[1]);
      begin wait (a17[1].u.p == 3) $display("%0t a17.0", $time); wait (a17[1].u.t == "ut") $display("%0t a17.1", $time); end
      t18(a18[1]);
      begin wait (a18[1].a[2] == 8) $display("%0t a18.0", $time); wait (a18[1].a[2] == 3) $display("%0t a18.1", $time); end
      t19(a19[1]);
      begin wait (a19[1].a[2] == 7) $display("%0t a19.0", $time); end
      t20(a20[1]);
      begin wait (a20[1].h != null) $display("%0t a20.0", $time); end
      t21(a21[1]);
      begin wait (a21[1].n == 88) $display("%0t a21.0", $time); end
      t22(a22[1]);
      begin wait (a22[1].n == 99) $display("%0t a22.0", $time); end
      t23(a23[1]);
      begin wait (a23[1].n == 98) $display("%0t a23.0", $time); end
      t24(a24[1]);
      begin wait (a24[1].m == 1) $display("%0t a24.0", $time); wait (a24[1].m == 2) $display("%0t a24.1", $time); end
      t25(a25[1]);
      begin wait (a25[1].n == 61) $display("%0t a25.0", $time); end
      t26(a26[1]);
      begin wait (a26[1].n == 1) $display("%0t a26.0", $time); end
      t27(a27[1]);
      begin wait (a27[1].n == 71) $display("%0t a27.0", $time); end
      t28(a28[1]);
      begin wait (a28[1].b == 1) $display("%0t a28.0", $time); end
      t29(a29[1]);
      begin wait (a29[1].n == 81) $display("%0t a29.0", $time); end
      t30(a30[1]);
      begin wait (a30[1].n == 82) $display("%0t a30.0", $time); end
      begin #30 $display("end"); $finish; end
    join
  end
endmodule
