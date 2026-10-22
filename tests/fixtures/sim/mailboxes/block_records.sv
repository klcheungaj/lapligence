module tb;
  typedef struct { logic [3:0] n; bit [1:0] p [2]; } in_t;
  typedef struct { int a; logic [7:0] l; in_t i; byte s; } S;
  mailbox #(S) ms = new();
  S mq[$];
  S da[];
  S aa[int];
  S got;
  initial begin
    begin
      S t;
      t.a = -5; t.l = 8'b10x1z0x1; t.i.n = 4'b1x0z; t.i.p[0] = 2'b10; t.i.p[1] = 2'b01; t.s = -3;
      ms.put(t);
      void'(ms.try_put(t));
      mq.push_back(t);
      mq.push_front(t);
      mq.insert(1, t);
      da = new[2];
      da[1] = t;
      aa[7] = t;
    end
    begin
      automatic S u;
      u.a = 9; u.l = 8'hA5; u.i.n = 4'h3; u.i.p[0] = 2'b11; u.i.p[1] = 2'b00; u.s = 4;
      ms.put(u);
      mq[0] = u;
    end
    while (ms.num() > 0) begin
      ms.get(got);
      $display("mb a=%0d l=%b n=%b p=%b%b s=%0d", got.a, got.l, got.i.n, got.i.p[0], got.i.p[1], got.s);
    end
    foreach (mq[k]) $display("q%0d a=%0d l=%b n=%b s=%0d", k, mq[k].a, mq[k].l, mq[k].i.n, mq[k].s);
    $display("da a=%0d l=%b p=%b%b", da[1].a, da[1].l, da[1].i.p[0], da[1].i.p[1]);
    $display("aa a=%0d n=%b", aa[7].a, aa[7].i.n);
    begin
      S v;
      ms.put(mq[1]);
      ms.get(v);
      $display("back a=%0d l=%b n=%b s=%0d", v.a, v.l, v.i.n, v.s);
      ms.put(mq[2]);
      ms.get(v);
      $display("got a=%0d l=%b", v.a, v.l);
    end
    $finish;
  end
endmodule
