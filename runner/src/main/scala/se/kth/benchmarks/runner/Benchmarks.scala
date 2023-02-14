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
  private val testNodes = List(3);
  private val testDuration= List(1*10);
  private val testConcurrentProposals = List(200L);

  private val nodes = List(3, 5);
  private val chainedNodes = List(3);
  private val quorumLossNodes = List(5);
  private val reconfigNodes = List(5);

  private val duration = List(5*60);
  private val ConcurrentProposals = List(500L, 5L.k, 50L.k);

  private val normalAlgorithms = List("paxos", "raft", "multi-paxos");
  private val pcAlgorithms = List("vr", "multi-paxos", "paxos", "raft", "raft_pv_qc");
  private val reconfigAlgorithms = List("paxos", "raft");

  private val reconfig_policy = List("replace-follower", "replace-leader");
  private val election_timeout_ms = List(50L, 500L, 5L.k)
  private val stable_election_timeout_ms = List(10L.k)

  private val normalTestSpace = ParameterSpacePB // test space
    .cross(
      List("paxos"),
      testNodes,
      testDuration,
      testConcurrentProposals,
      List("off"),
      List("none"),
      List("fully_connected"),
      stable_election_timeout_ms
    );

  private val normalThreeSpace = ParameterSpacePB
    .cross(
      normalAlgorithms,
      List(3),
      duration,
      ConcurrentProposals,
      List("off"),
      List("none"),
      List("fully_connected"),
      stable_election_timeout_ms
    );

  private val normalFiveSpace = ParameterSpacePB
    .cross(
      normalAlgorithms,
      List(5),
      duration,
      ConcurrentProposals,
      List("off"),
      List("none"),
      List("fully_connected"),
      stable_election_timeout_ms
    );

  private val chainedSpace = ParameterSpacePB
    .cross(
      pcAlgorithms,
      chainedNodes,
      duration,
      List(500L),
      List("off"),
      List("none"),
      List("chained-60", "chained-120", "chained-240"),
      election_timeout_ms
    );

  private val quorumLossConstrainedSpace = ParameterSpacePB
    .cross(
      pcAlgorithms,
      quorumLossNodes,
      duration,
      List(500L),
      List("off"),
      List("none"),
      List("quorum_loss-60", "quorum_loss-120", "quorum_loss-240", "constrained_election-60", "constrained_election-120", "constrained_election-240"),
      election_timeout_ms
    );

  private val reconfigSingleSpace = ParameterSpacePB
    .cross(
      reconfigAlgorithms,
      reconfigNodes,
      duration,
      List(500L, 50L.k),
      List("single"),
      reconfig_policy,
      List("fully_connected"),
      stable_election_timeout_ms
    );

  private val reconfigMajoritySpace = ParameterSpacePB
    .cross(
      reconfigAlgorithms,
      reconfigNodes,
      duration,
      List(50L.k),
      List("majority"),
      reconfig_policy,
      List("fully_connected"),
      stable_election_timeout_ms
    );

  private val partialConnectivitySpace = chainedSpace.append(quorumLossConstrainedSpace);
  private val normalSpace = normalThreeSpace.append(normalFiveSpace)
  private val reconfigSpace = reconfigSingleSpace.append(reconfigMajoritySpace);

  val atomicBroadcast = Benchmark(
    name = "Atomic Broadcast",
    symbol = "ATOMICBROADCAST",
    invoke = (stub, request: AtomicBroadcastRequest) => {
      stub.atomicBroadcast(request)
    },
    space = partialConnectivitySpace.append(normalSpace)
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
    testSpace = normalTestSpace
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
