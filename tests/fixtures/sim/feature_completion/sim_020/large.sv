// SIM-020 A03: streams of 1.6M and 3.2M bits exceed the packed value width
// and stay in runtime-sized segments end to end.
module tb;
  localparam int N = 200000;
  byte q[$], r[$];
  bit [7:0] f [0:N-1];
  bit [7:0] g [0:N-1];
  int w[];
  string s;
  longint sum;
  initial begin
    for (int k = 0; k < N; k++) q.push_back(k[7:0] ^ 8'h5a);
    r = {<<8{q}};
    sum = 0;
    foreach (r[k]) sum += r[k] * (k % 7 + 1);
    $display("queue-reverse %0d %h %h %0d", r.size(), r[0], r[N-1], sum);
    f = {<<8{q}};
    sum = 0;
    foreach (f[k]) sum += f[k] * (k % 7 + 1);
    $display("queue-to-fixed %h %h %0d", f[0], f[N-1], sum);
    {<<8{r}} = f;
    $display("fixed-unpack %0d %h %h %0d", r.size(), r[0], r[N-1], r == q);
    w = {>>{q, f}};
    sum = 0;
    foreach (w[k]) sum += w[k] % 1000;
    $display("two-segments %0d %h %h %0d", w.size(), w[0], w[2*N/4-1], sum);
    {>>{g}} = {<<8{f}};
    $display("fixed-to-fixed %h %h", g[0], g[N-1]);
    q = {};
    for (int k = 0; k < N; k++) q.push_back(8'h41 + k % 26);
    s = {>>{q}};
    $display("large-string %0d %s %s", s.len(), s.substr(0, 3), s.substr(N-2, N-1));
    $finish;
  end
endmodule
