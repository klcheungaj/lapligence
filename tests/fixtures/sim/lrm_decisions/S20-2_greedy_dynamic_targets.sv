// IEEE 1800-2009 11.4.14.4 L15600-15602: "If the unpack operation includes
// unbounded dynamically sized types, the process is greedy (as in a cast): the
// first dynamically sized item is resized to accept all the available data
// (excluding subsequent fixed-size items) in the stream; any remaining
// dynamically sized items are left empty."
// Decision (LRM text): with several unselected dynamic arrays or queues in
// one unpack, the first takes every bit the fixed targets after it leave and
// each later one becomes empty, whatever size it had.
module tb;
  byte h, t;
  byte d[];
  byte q[$];
  initial begin
    q.push_back(8'h09);
    {>>{h, d, q, t}} = 40'haabbccddee;
    $display("first %h %0d %h %h %0d %h", h, d.size(), d[0], d[2], q.size(), t);
    {>>{h, d, t}} = 16'h0102;
    $display("empty %h %0d %h", h, d.size(), t);
    $finish;
  end
endmodule
