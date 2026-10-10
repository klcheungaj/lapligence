// IEEE 1800-2009 6.24.3 L7514: "When a dynamic array, queue, or string type is
// converted to the packed representation, the item at index 0 occupies the
// MSBs." L7528: "For the purposes of a bit-stream cast, a string type is
// considered a dynamic array of bytes." 6.16 L5641-5642: "A string variable
// shall not contain the special character "\0". Assigning the value 0 to a
// string character shall be ignored." 11.4.14 L15472-15474: a dynamically
// sized target "is resized to accommodate the entire stream".
// Decision (LRM text): a string streams as its bytes, index 0 leftmost; a
// stream assigned or unpacked into a string fills it left to right, one
// character per 8 bits, and zero bytes are dropped.
module tb;
  string s, t;
  byte q[$];
  logic [31:0] v;
  initial begin
    s = "ab";
    t = "cd";
    v = {<<8{s, t}};
    $display("reversed %h", v);
    q = {>>{s}};
    $display("bytes %0d %h %h", q.size(), q[0], q[1]);
    s = {>>{16'h4100, 8'h42}};
    $display("pack %s %0d", s, s.len());
    {>>{s}} = 24'h43_00_44;
    $display("unpack %s %0d", s, s.len());
    {<<8{t}} = 32'h41424344;
    $display("unpack-reversed %s", t);
    $finish;
  end
endmodule
