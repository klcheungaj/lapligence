// IEEE 1800-2009 7.12.3: reduction methods apply to unpacked arrays of
// integral values; sum() over a real array is rejected by the frontend even
// with a real `with` map (adopted FND-002 real_map witness).
module tb;
real a[2]; real r;
initial begin
a='{1.25,2.5}; r=a.sum() with (item*2.0); $display("%.2f",r);
$finish(0);
end
endmodule
