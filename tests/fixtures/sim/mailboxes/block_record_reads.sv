module tb;
  typedef struct { logic [3:0] n; bit [1:0] p [2]; } in_t;
  typedef struct { int a; logic [7:0] l; in_t i; byte s; } S;
  S mq[$];
  S da[];
  S aa[int];
  initial begin
    begin
      S t;
      t.a = -5; t.l = 8'b10x1z0x1; t.i.n = 4'b1x0z; t.i.p[0] = 2'b10; t.i.p[1] = 2'b01; t.s = -3;
      mq.push_back(t);
      t.a = 7; t.l = 8'hA5; t.i.n = 4'h3; t.s = 4;
      mq.push_back(t);
      da = new[2];
      da[1] = t;
      aa[7] = mq[0];
    end
    begin
      S r;
      r = mq[1];
      $display("q1 a=%0d l=%b n=%b p=%b%b s=%0d", r.a, r.l, r.i.n, r.i.p[0], r.i.p[1], r.s);
      r = da[1];
      $display("da a=%0d l=%b s=%0d", r.a, r.l, r.s);
      r = aa[7];
      $display("aa a=%0d l=%b n=%b", r.a, r.l, r.i.n);
      r = mq.pop_front();
      $display("pop a=%0d l=%b size=%0d", r.a, r.l, mq.size());
      // A missing key reads the element type's default value.
      r = aa[3];
      $display("missing a=%0d l=%b p=%b%b exists=%0d", r.a, r.l, r.i.p[0], r.i.p[1], aa.exists(3));
    end
    begin
      automatic S u;
      u = da[0];
      $display("auto a=%0d l=%b", u.a, u.l);
    end
    $finish;
  end
endmodule
