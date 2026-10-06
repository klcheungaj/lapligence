// Readers of a record member container wake on its contents and shape
// changes (SV 9.2.2.2): `always_comb` re-evaluates after a push and a write.
module tb;
 typedef struct { string s; int q[$]; } rec_t;
 rec_t m; int n, f;
 always_comb n = m.q.size();
 always_comb f = (m.q.size() > 0) ? m.q[0] : -1;
 initial begin
 #1 $display("%0d %0d", n, f);
 m.q.push_back(4);
 #1 $display("%0d %0d", n, f);
 m.q[0] = 6;
 #1 $display("%0d %0d", n, f);
 $finish(0);
 end
endmodule
