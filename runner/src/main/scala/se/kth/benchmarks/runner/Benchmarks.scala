package se.kth.benchmarks.runner

import kompics.benchmarks.benchmarks._
import kompics.benchmarks.messages._
import scala.concurrent.Future
import scala.util.{Failure, Success, Try}
import com.lkroll.common.macros.Macros
import scala.concurrent.duration._

case class BenchmarkRun[Params](name: String, symbol: String, invoke: (Runner.Stub, Params) => Future[TestResult]);

trait Benchmark {
  def name: String;
  def symbol: String;
  def withStub(stub: Runner.Stub, testing: Boolean)(f: (Future[TestResult], ParameterDescription, Long) => Unit): Unit;
  def requiredRuns(testing: Boolean): Long;
}
object Benchmark {
  def apply[Params](b: BenchmarkRun[Params],
                    space: ParameterSpace[Params],
                    testSpace: ParameterSpace[Params]): BenchmarkWithSpace[Params] =
    BenchmarkWithSpace(b, space, testSpace);
  def apply[Params](name: String,
                    symbol: String,
                    invoke: (Runner.Stub, Params) => Future[TestResult],
                    space: ParameterSpace[Params],
                    testSpace: ParameterSpace[Params]): BenchmarkWithSpace[Params] =
    BenchmarkWithSpace(BenchmarkRun(name, symbol, invoke), space, testSpace);
}
case class BenchmarkWithSpace[Params](b: BenchmarkRun[Params],
                                      space: ParameterSpace[Params],
                                      testSpace: ParameterSpace[Params])
    extends Benchmark {
  override def name: String = b.name;
  override def symbol: String = b.symbol;
  def run = b.invoke;
  override def withStub(stub: Runner.Stub,
                        testing: Boolean)(f: (Future[TestResult], ParameterDescription, Long) => Unit): Unit = {
    var index = 0L;
    val useSpace = if (testing) testSpace else space;
    useSpace.foreach { p =>
      index += 1L;
      f(run(stub, p), useSpace.describe(p), index)
    }
  }
  override def requiredRuns(testing: Boolean): Long = if (testing) testSpace.size else space.size;
}

object Benchmarks extends ParameterDescriptionImplicits {

  implicit class ExtLong(i: Long) {
    def mio: Long = i * 1000000L;
    def k: Long = i * 1000L;
  }

  //implicit def seq2param[T: ParameterDescriptor](s: Seq[T]): ParameterSpace[T] = ParametersSparse1D(s);

  /*** split into different parameter spaces as some parameters are dependent on each other ***/
  private val atomicBroadcastTestNodes = List(3);
  private val atomicBroadcastTestDuration= List(1*60);
  private val atomicBroadcastTestConcurrentProposals = List(200L);

  private val atomicBroadcastNodes = List(3);
  private val atomicBroadcastDuration = List(5*60);
  private val atomicBroadcastConcurrentProposals = List(500L, 5L.k, 50L.k);

  private val algorithms = List("vr", "multi-paxos", "paxos", "raft", "raft_pv_qc");
  private val reconfig = List("single", "majority");
  private val reconfig_policy = List("replace-follower", "replace-leader");
  private val network_scenarios = List("fully_connected", "quorum_loss-60", "constrained_election-60", "chained-60")
  private val election_timeout_ms = List(10L.k)

  private val atomicBroadcastNormalTestSpace = ParameterSpacePB // test space without reconfig
    .cross(
      algorithms,
      atomicBroadcastTestNodes,
      atomicBroadcastTestDuration,
      atomicBroadcastTestConcurrentProposals,
      List("off"),
      List("none"),
      network_scenarios,
      election_timeout_ms
    );

  private val atomicBroadcastReconfigTestSpace = ParameterSpacePB // test space with reconfig
    .cross(
      algorithms,
      atomicBroadcastTestNodes,
      atomicBroadcastTestDuration,
      atomicBroadcastTestConcurrentProposals,
      reconfig,
      reconfig_policy,
      List("fully_connected"),
      election_timeout_ms
    );

  private val atomicBroadcastTestSpace = atomicBroadcastNormalTestSpace.append(atomicBroadcastReconfigTestSpace);

  private val atomicBroadcastNormalSpace = ParameterSpacePB
    .cross(
      List("paxos", "raft", "raft_pv_qc", "vr"),
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      List(500L),
      List("off"),
      List("none"),
      List("chained-60", "chained-120", "chained-240"),
      List(5L, 50L, 500L, 5L.k)
    );

  private val multiPaxosNormalSpace = ParameterSpacePB
    .cross(
      List("multi-paxos"),
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      atomicBroadcastConcurrentProposals,
      List("off"),
      List("none"),
      List("fully_connected"),
      election_timeout_ms
    );

  private val atomicBroadcastReconfigSpace = ParameterSpacePB
    .cross(
      algorithms,
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      atomicBroadcastConcurrentProposals,
      reconfig,
      reconfig_policy,
      network_scenarios,
      election_timeout_ms
    );

  private val atomicBroadcastSpace = multiPaxosNormalSpace.append(atomicBroadcastNormalSpace);

  private val latencySpace = ParameterSpacePB
    .cross(
      algorithms,
      List(3),
      List(1L.k),
      List(1L),
      List("off"),
      List("none"),
      network_scenarios,
      election_timeout_ms
    );

  val atomicBroadcast = Benchmark(
    name = "Atomic Broadcast",
    symbol = "ATOMICBROADCAST",
    invoke = (stub, request: AtomicBroadcastRequest) => {
      stub.atomicBroadcast(request)
    },
    space = atomicBroadcastSpace
      .msg[AtomicBroadcastRequest] {
        case (a, nn, d, cp, r, rp, ns, et) =>
          AtomicBroadcastRequest(
            algorithm = a,
            numberOfNodes = nn,
            durationSecs = d,
            concurrentProposals = cp,
            reconfiguration = r,
            reconfigPolicy = rp,
            networkScenario = ns,
            electionTimeoutMs = et,
          )
      },
    testSpace = atomicBroadcastNormalTestSpace
      .msg[AtomicBroadcastRequest] {
        case (a, nn, d, cp, r, rp, ns, et) =>
          AtomicBroadcastRequest(
            algorithm = a,
            numberOfNodes = nn,
            durationSecs = d,
            concurrentProposals = cp,
            reconfiguration = r,
            reconfigPolicy = rp,
            networkScenario = ns,
            electionTimeoutMs = et,
          )
      }
  );

  val benchmarks: List[Benchmark] = Macros.memberList[Benchmark];
  lazy val benchmarkLookup: Map[String, Benchmark] = benchmarks.map(b => (b.symbol -> b)).toMap;
}
