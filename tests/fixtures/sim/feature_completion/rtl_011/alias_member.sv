// SV2009 10.11, Annex A.6.1
// Expected: 1 | 
// Adopted FND-002 witness L-F08-05-02/L-F08-05-03 (alias_member).
module tb;
typedef struct packed {logic a,b;} T; wire T x; wire y; alias x.a=y; assign y=1;
initial begin
#1; $display("%b",x.a);
$finish(0);
end
endmodule
