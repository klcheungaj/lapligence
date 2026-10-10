// SIM-020 A01/A02: resizable unpack targets, greedy extents, `with` ranges,
// left-aligned zero fill, string zero bytes and records with dynamic members.
module tb;
  typedef struct { byte len; byte payload[]; byte crc; } pkt_t;
  typedef shortint sd_t[];
  byte h, t, len, crc;
  byte d[];
  byte q2[$];
  byte q[$];
  byte payload[];
  shortint w[];
  sd_t sd;
  string s;
  pkt_t pk, r;
  initial begin
    {>>{h, d, t}} = 40'h0102030405;
    $display("greedy-middle %h %0d %h %h %h %h", h, d.size(), d[0], d[1], d[2], t);
    q2.push_back(8'h09);
    {>>{h, d, q2, t}} = 32'haabbccdd;
    $display("greedy-first %h %0d %0d %h %h %h", h, d.size(), q2.size(), d[0], d[1], t);

    q = {8'h03, 8'h01, 8'h02, 8'h03, 8'h77, 8'h99};
    {>>{len, payload with [0 +: len], crc}} = q;
    $display("with-length %0d %0d %h %h %h %0d", len, payload.size(), payload[0], payload[2], crc,
             q.size());
    q = {};
    {>>{q with [1:2]}} = 16'haabb;
    $display("with-range %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    q = {8'h10, 8'h11, 8'h12, 8'h13};
    {>>{q with [2]}} = 8'h5a;
    $display("with-index %0d %h %h %h", q.size(), q[1], q[2], q[3]);

    w = {>>{24'h010203}};
    $display("zero-fill %0d %h %h", w.size(), w[0], w[1]);
    sd = sd_t'(32'h01020304);
    $display("cast-to-dynamic %0d %h %h", sd.size(), sd[0], sd[1]);

    s = {>>{16'h4100, 8'h42}};
    $display("string-zero-bytes %s %0d", s, s.len());
    {>>{s}} = 24'h43_00_44;
    $display("string-unpack %s %0d", s, s.len());

    pk.len = 2;
    pk.payload = new[2];
    pk.payload[0] = 8'haa;
    pk.payload[1] = 8'hbb;
    pk.crc = 8'h55;
    q = {>>{pk}};
    $display("record-source %0d %h %h %h %h", q.size(), q[0], q[1], q[2], q[3]);
    {>>{r.len, r.payload with [0 +: r.len], r.crc}} = q;
    $display("record-members %0d %0d %h %h %h", r.len, r.payload.size(), r.payload[0],
             r.payload[1], r.crc);
    $finish;
  end
endmodule
