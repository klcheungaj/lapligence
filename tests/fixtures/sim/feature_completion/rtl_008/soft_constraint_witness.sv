// SV2009 18.5 Annex A.1.9
// Expected: required diagnostic
module tb;
class C; rand int x; constraint c{soft x==7;} endclass C a;
initial begin
a=new;
$finish;
end
endmodule
