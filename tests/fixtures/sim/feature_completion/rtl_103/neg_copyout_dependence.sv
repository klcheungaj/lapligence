// Owner policy (as for nonblocking unpacks, RTL-015): a copy-out actual's
// `with` selectors are fixed when the call starts, so a selector cannot read
// a target unpacked earlier by the same copy-out (IEEE 1800-2009 11.4.14.4
// lets a direct blocking unpack see it). Assign the length first.
module tb;
  logic [7:0] arr [0:7];
  logic [7:0] len;
  task automatic get(output logic [31:0] v);
    v = 32'h03112233;
  endtask
  initial begin
    get({>>{len, arr with [0 +: len]}});
    $finish(0);
  end
endmodule
