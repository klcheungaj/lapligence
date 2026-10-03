// SV2009 7.2.1,12.6
// Expected: match
module tb;
typedef struct packed {bit [1:0] b; logic [1:0] l;} T; T x;
initial begin
x=4'bxx01; if(x matches '{2'b00,2'b01}) $display("match"); else $display("bad");
$finish(0);
end
endmodule
