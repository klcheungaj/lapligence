// SV2009 3.12.1, 23.10, 26.3: a module in another file calls the shared
// package subprogram from a declaration initializer of each instance.
module shared_user #(parameter int N = 1) (output int calls_seen);
  int first = shared_pkg::take(N);
  assign calls_seen = first;
endmodule
