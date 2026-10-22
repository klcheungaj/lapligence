// IEEE 1800-2009 11.4.14.3 L15561-15565: "When a streaming_concatenation
// appears as the target of an assignment, the streaming operators perform the
// reverse operation ... If the source expression contains more bits than are
// needed, the appropriate number of bits shall be consumed from its left (most
// significant) end."
// IEEE 1800-2009 11.4.14.4 L15623-15625: "The expression within the with is
// evaluated immediately before its corresponding array is streamed (i.e.,
// packed or unpacked). Thus, the expression can refer to data that are
// unpacked by the same operator but before the array."
// Decision (industry-standard behaviour): a `<<` unpack into fixed-size
// targets only (a fixed array with a `with` range included) first takes the
// bits its targets need from the left of the source, then reverses those
// blocks. When a target is a queue or dynamic array, with or without a
// `with` range, the whole source is reversed first and the targets take the
// reversed stream from its left, each `with` range evaluated just before its
// array; bits left over at the right are not used.
module tb;
  byte a, b;
  byte q[$];
  byte fa[3];
  byte len;
  initial begin
    {<<8{a, b}} = 24'h112233;
    $display("fixed %h %h", a, b);
    {<<8{q with [0 +: 2]}} = 24'h112233;
    $display("selected %0d %h %h", q.size(), q[0], q[1]);
    q = {};
    {<<8{a, q with [0 +: 1]}} = 24'h112233;
    $display("mixed %h %0d %h", a, q.size(), q[0]);
    fa = '{0, 0, 0};
    {<<8{fa with [0 +: 2]}} = 24'h112233;
    $display("fixed array %h %h %h", fa[0], fa[1], fa[2]);
    len = 0;
    q = {};
    {<<8{len, q with [0 +: len]}} = 32'h44332202;
    $display("dependent %0d %0d %h %h", len, q.size(), q[0], q[1]);
    {<<8{q}} = 24'h112233;
    $display("whole %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    $finish;
  end
endmodule
