// IEEE 1800-2009 11.4.14.3 L15561-15565: "When a streaming_concatenation
// appears as the target of an assignment, the streaming operators perform the
// reverse operation ... If the source expression contains more bits than are
// needed, the appropriate number of bits shall be consumed from its left (most
// significant) end."
// Decision (llg choice): a `<<` unpack first takes the bits its targets need
// from the left of the source, then reverses those blocks; `with` extents are
// resolved before the reordering, so a selected queue receives the leftmost
// consumed blocks in reverse.
module tb;
  byte a, b;
  byte q[$];
  initial begin
    {<<8{a, b}} = 24'h112233;
    $display("fixed %h %h", a, b);
    {<<8{q with [0 +: 2]}} = 24'h112233;
    $display("selected %0d %h %h", q.size(), q[0], q[1]);
    {<<8{q}} = 24'h112233;
    $display("whole %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    $finish;
  end
endmodule
