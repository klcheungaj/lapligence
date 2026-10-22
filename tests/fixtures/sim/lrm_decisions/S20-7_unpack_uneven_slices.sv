// IEEE 1800-2009 11.4.14.2 L15543-15546: "For right-to-left streaming using
// <<, the stream is sliced into blocks with the specified number of bits,
// starting with the right-most bit. If as a result of slicing the last
// (left-most) block has fewer bits than the block size, the last block has
// the size of the remaining bits; there is no padding or truncation."
// IEEE 1800-2009 11.4.14.3 L15561-15562: "When a streaming_concatenation
// appears as the target of an assignment, the streaming operators perform
// the reverse operation; i.e. to unpack a stream of bits into one or more
// variables."
// IEEE 1800-2009 11.4.14.4 L15600-15602: "the first dynamically sized item
// is resized to accept all the available data (excluding subsequent
// fixed-size items) in the stream".
// Decision (industry-standard behaviour): a `<<` unpack reorders the source
// bits it consumes exactly as a pack does: blocks are sliced starting with
// the right-most source bit, so a short block is the left-most block of the
// source, and the block order is reversed; the targets then take the
// reordered stream from its left. When the slice size divides the consumed
// width this is the inverse of the pack. A queue, dynamic array or string
// that takes the whole source is resized to hold every source bit; a last
// partial element is zero-filled on the right.
module tb;
  logic [7:0] lanes [0:2];
  logic [7:0] a, b, c;
  logic [3:0] n;
  logic [10:0] w;
  byte q[$];
  bit [4:0] d[];
  string s;
  initial begin
    {<<5{lanes}} = 24'h1d_98_3a;
    $display("fixed array %h %h %h", lanes[0], lanes[1], lanes[2]);
    {<<5{a, b, c}} = 24'h1d_98_3a;
    $display("variables %h %h %h", a, b, c);
    {<<5{a, n}} = 16'hbeef;
    $display("wider source %h %h", a, n);
    w = 11'h5a3;
    {<<4{a, n}} = {<<3{w, 1'b1}};
    $display("nested %h %h", a, n);
    {<<3{q}} = 24'h1d_98_3a;
    $display("queue whole %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    {<<3{q}} = 20'h1d_98_3;
    $display("queue partial %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    {<<4{d}} = 15'h5a5a;
    $display("dynamic %0d %h %h %h", d.size(), d[0], d[1], d[2]);
    {>>{q}} = 12'h5a3;
    $display("left to right %0d %h %h", q.size(), q[0], q[1]);
    {<<5{s}} = 20'h41424;
    $display("string %0d %h", s.len(), s);
    $finish;
  end
endmodule
