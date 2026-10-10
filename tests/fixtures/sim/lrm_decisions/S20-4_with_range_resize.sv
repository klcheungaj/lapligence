// IEEE 1800-2009 11.4.14.4 L15629-15630: "When used within the context of an
// unpack operation and the array is a variable-size array, it shall be resized
// to accommodate the range expression." L15643-15645: "If the range expression
// evaluates to a range smaller than the extent of the array (fixed or variable
// size), only the specified items are unpacked into the designated array
// locations; the remainder of the array is unmodified."
// Decision (llg choice): a `with` range grows a queue or dynamic array when
// the range reaches past its end (new elements read as zero) and never shrinks
// it; elements outside the range keep their values.
module tb;
  byte q[$];
  byte p[$];
  byte d[];
  initial begin
    q = {8'h01, 8'h02, 8'h03, 8'h04};
    {>>{q with [0 +: 2], p}} = 32'haabbccdd;
    $display("kept %0d %h %h %h %h %0d", q.size(), q[0], q[1], q[2], q[3], p.size());
    {>>{d with [1:2]}} = 16'h1122;
    $display("grown %0d %h %h %h", d.size(), d[0], d[1], d[2]);
    $finish;
  end
endmodule
