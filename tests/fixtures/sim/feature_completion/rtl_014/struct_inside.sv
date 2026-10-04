// SV2009 6.4,11.4.13
// Expected: required diagnostic
module tb;
typedef struct {int a; string s;} T; T x,y;
initial begin
x='{7,"ok"}; y=x; $display("%b",x inside {y});
$finish;
end
endmodule
