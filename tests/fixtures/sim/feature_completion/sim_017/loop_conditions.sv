// SIM-017: expression-form try_get/try_peek/try_put of an aggregate or
// selected operand in loop conditions, a loop step, short-circuited operands
// and ?: arms. The retrieval and its copy-out run at every evaluation of the
// condition, before the body, and never in an operand that is not evaluated
// (IEEE 1800-2009 12.7.1 L18071-18072, 12.7.4 L18245, 12.7.5 L18283,
// 11.3.5 L14430-14434, 11.4.7 L14896-14899, 11.4.11 L15147-15148). Leaf
// record (msg_t) and column record (big_t, 21 cells) messages, a bounded
// try_put, a selected scalar destination, event messages, an automatic
// class-task local and a body that suspends.
typedef struct { int a; string s; } msg_t;
typedef struct { int v[20]; int tag; } big_t;

class drainer;
  mailbox #(msg_t) box;
  function new(mailbox #(msg_t) b); box = b; endfunction
  task drain(output int count);
    msg_t r;
    count = 0;
    while (box.try_get(r) > 0) begin
      count++;
      $display("class %0d %s", r.a, r.s);
    end
  endtask
endclass

module tb;
  mailbox #(msg_t) mb = new();
  mailbox #(msg_t) bounded = new(2);
  mailbox #(big_t) bb = new();
  mailbox ub = new();
  mailbox #(int) ib = new();
  msg_t m, got;
  big_t b, bgot;
  event e1, e2, eg;
  drainer d;
  int flag = 0;
  int arr[3];
  int i, k, n;

  initial begin
    m.a = 1; m.s = "x"; mb.put(m);
    m.a = 2; m.s = "y"; mb.put(m);
    m.a = 3; m.s = "z"; mb.put(m);
    for (i = 0; mb.try_get(got) > 0; i++) $display("for %0d %0d %s", i, got.a, got.s);
    $display("after for i %0d num %0d", i, mb.num());

    m.a = 4; m.s = "p"; mb.put(m);
    m.a = 5; m.s = "q"; mb.put(m);
    n = 0;
    do begin n++; $display("do %0d %0d %s", n, got.a, got.s); end while (mb.try_get(got) > 0);
    $display("after do n %0d num %0d got %0d", n, mb.num(), got.a);

    m.a = 6; m.s = "r"; mb.put(m);
    got.a = 0; got.s = "-";
    if (flag != 0 && mb.try_get(got) > 0) $display("taken");
    $display("and skip num %0d got %0d %s", mb.num(), got.a, got.s);
    i = flag != 0 ? mb.try_get(got) : -5;
    $display("arm skip i %0d num %0d got %0d", i, mb.num(), got.a);
    $display("display arm %0d num %0d", flag != 0 ? mb.try_peek(got) : 7, mb.num());
    flag = 1;
    i = flag != 0 ? mb.try_peek(got) : -5;
    $display("arm taken i %0d num %0d got %0d %s", i, mb.num(), got.a, got.s);
    if (flag != 0 || mb.try_get(got) > 0) $display("or skip num %0d", mb.num());
    $display("peek condition %0d", mb.try_peek(got) > 0 ? got.a : -1);
    n = 0;
    while (n < 3 && mb.try_get(got) > 0) begin n++; $display("guarded %0d %s", got.a, got.s); end
    $display("guarded n %0d num %0d", n, mb.num());

    for (k = 0; k < 20; k++) b.v[k] = k;
    for (k = 0; k < 2; k++) begin b.v[19] = k + 10; b.tag = k; bb.put(b); end
    n = 0;
    while (bb.try_get(bgot) > 0) begin n++; $display("big %0d %0d %0d", bgot.tag, bgot.v[0], bgot.v[19]); end
    $display("big n %0d", n);

    m.a = 20; m.s = "b";
    n = 0;
    while (bounded.try_put(m) > 0) begin n++; m.a++; end
    $display("bounded n %0d num %0d", n, bounded.num());

    ib.put(7); ib.put(8);
    i = 0;
    while (ib.try_get(arr[i]) > 0) i++;
    $display("arr %0d %0d %0d i %0d", arr[0], arr[1], arr[2], i);

    ub.put(e1); ub.put(e2);
    n = 0;
    while (ub.try_get(eg) > 0) n++;
    $display("events n %0d num %0d", n, ub.num());

    m.a = 30; m.s = "c"; mb.put(m);
    m.a = 31; m.s = "d"; mb.put(m);
    d = new(mb);
    d.drain(n);
    $display("class n %0d num %0d", n, mb.num());

    fork
      begin
        #2 m.a = 40; m.s = "e"; mb.put(m);
        #2 m.a = 41; m.s = "f"; mb.put(m);
      end
    join_none
    #1;
    n = 0;
    while (n < 2) begin
      while (mb.try_get(got) > 0) begin
        n++;
        #1 $display("t %0d got %0d %s", $time, got.a, got.s);
      end
      #1;
    end
    $display("timed n %0d num %0d", n, mb.num());
    $finish;
  end
endmodule
